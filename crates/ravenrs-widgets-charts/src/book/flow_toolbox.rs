//! The stories of `FlowToolbox`, the list of blocks to drag onto a `FlowCanvas`. Written by hand:
//! it has no example in the gallery.

use ravenrs_widgets_base::AnyView;
use ravenrs_widgets_base::book::{ArgType, Args, ComponentBook, Meta};

use crate::{FlowNode, FlowPort, FlowToolbox};

fn render(args: &Args) -> AnyView {
    let mut toolbox = FlowToolbox::new()
        .group("Triggers")
        .model(
            "New sale",
            FlowNode::new("sale", "trigger", "New sale", 0.0, 0.0).port(FlowPort::output("out")),
        )
        .group("Actions")
        .model(
            "Check stock",
            FlowNode::new("stock", "action", "Check stock", 0.0, 0.0).port(FlowPort::input("in")),
        )
        .model(
            "Issue receipt",
            FlowNode::new("invoice", "action", "Issue receipt", 0.0, 0.0).port(FlowPort::input("in")),
        )
        .searchable(args.bool("searchable"))
        .width(args.number("width") as f32);
    if !args.text("title").is_empty() {
        toolbox = toolbox.title(args.text("title"));
    }
    Box::new(toolbox)
}

impl ComponentBook for FlowToolbox {
    fn book() -> Meta {
        Meta::new("Data/FlowToolbox", render)
            .summary("The blocks a person drags onto a FlowCanvas, in groups, with a search.")
            .arg(ArgType::text("title", "").description("Empty: no title."))
            .arg(ArgType::bool("searchable", true))
            .arg(ArgType::number("width", 240.0, 160.0, 400.0, 8.0))
            .story("Blocks", Args::new())
            .story("With title", Args::new().with("title", "Blocks"))
            .story("No search", Args::new().with("searchable", false))
    }
}
