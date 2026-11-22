//! The stories of `FlowLibrary`, the diagrams kept beside a `FlowCanvas`. Written by hand: it has
//! no example in the gallery.

use ravenrs_reactive::State;
use ravenrs_widgets_base::AnyView;
use ravenrs_widgets_base::book::{ArgType, Args, ComponentBook, Meta};

use super::flow_graph_sample::graph;
use crate::FlowLibrary;

fn render(args: &Args) -> AnyView {
    let graph = State::new(graph());
    let mut library = FlowLibrary::new(graph.binding()).width(args.number("width") as f32);
    if !args.text("title").is_empty() {
        library = library.title(args.text("title"));
    }
    Box::new(library)
}

impl ComponentBook for FlowLibrary {
    fn book() -> Meta {
        Meta::new("Data/FlowLibrary", render)
            .summary("The diagrams kept beside a FlowCanvas: open, save, rename and start new ones.")
            .arg(ArgType::text("title", "").description("Empty: no title."))
            .arg(ArgType::number("width", 240.0, 160.0, 400.0, 8.0))
            .story("Library", Args::new())
            .story("With title", Args::new().with("title", "Scenarios"))
    }
}
