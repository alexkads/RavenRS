//! The stories of `FlowLayers`, the outline of a `FlowCanvas`'s nodes. Written by hand: it has no
//! example in the gallery.

use ravenrs_reactive::State;
use ravenrs_widgets_base::AnyView;
use ravenrs_widgets_base::book::{ArgType, Args, ComponentBook, Meta};

use super::flow_graph_sample::graph;
use crate::{FlowLayers, FlowSelection};

fn render(args: &Args) -> AnyView {
    let graph = State::new(graph());
    let selection = State::new(FlowSelection::default());
    Box::new(FlowLayers::new(graph.binding(), selection.binding()).width(args.number("width") as f32))
}

impl ComponentBook for FlowLayers {
    fn book() -> Meta {
        Meta::new("Data/FlowLayers", render)
            .summary("The nodes of a diagram as a list, to find, select and reorder them.")
            .arg(ArgType::number("width", 240.0, 160.0, 400.0, 8.0))
            .story("Three nodes", Args::new())
    }
}
