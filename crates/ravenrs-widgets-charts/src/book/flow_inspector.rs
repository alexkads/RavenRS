//! The stories of `FlowInspector`, the settings of what is selected on a `FlowCanvas`. Written by
//! hand: it has no example in the gallery.

use ravenrs_reactive::State;
use ravenrs_widgets_base::AnyView;
use ravenrs_widgets_base::book::{ArgType, Args, ComponentBook, Meta};

use super::flow_graph_sample::graph;
use crate::{FlowInspector, FlowSelection};

fn render(args: &Args) -> AnyView {
    let graph = State::new(graph());
    let mut chosen = FlowSelection::default();
    if args.bool("node_selected") {
        chosen.nodes.push("stock".to_owned());
    }
    let selection = State::new(chosen);
    Box::new(
        FlowInspector::new(graph.binding(), selection.binding())
            .read_only(args.bool("read_only"))
            .width(args.number("width") as f32),
    )
}

impl ComponentBook for FlowInspector {
    fn book() -> Meta {
        Meta::new("Data/FlowInspector", render)
            .summary("The settings of the node or connection selected on a FlowCanvas.")
            .arg(
                ArgType::bool("node_selected", true)
                    .description("A node chosen on the canvas; off, the panel says to choose one."),
            )
            .arg(ArgType::bool("read_only", false))
            .arg(ArgType::number("width", 288.0, 200.0, 480.0, 8.0))
            .story("A node", Args::new())
            .story("Nothing selected", Args::new().with("node_selected", false))
            .story("Read only", Args::new().with("read_only", true))
    }
}
