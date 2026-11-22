//! The stories of `FloatingPanel`. Written by hand: the gallery's example keeps whether it is
//! open inside a view of its own, which the scaffold does not take apart.

use ravenrs_reactive::State;
use ravenrs_widgets_base::book::{ArgType, Args, ComponentBook, Meta};
use ravenrs_widgets_base::{AnyView, Text};

use crate::FloatingPanel;

fn render(args: &Args) -> AnyView {
    let open = State::new(args.bool("open"));
    let minimized = State::new(args.bool("minimized"));
    Box::new(
        FloatingPanel::new(open.binding(), args.text("title"), || {
            Box::new(Text::new("The app stays usable behind this panel."))
        })
        .minimized(minimized.binding())
        .size(args.number("width") as f32, args.number("height") as f32),
    )
}

impl ComponentBook for FloatingPanel {
    fn book() -> Meta {
        Meta::new("Forms/FloatingPanel", render)
            .summary("A panel that floats over the app in a corner, for an assistant or a chat.")
            .arg(ArgType::text("title", "Assistant"))
            .arg(ArgType::bool("open", true).description("Closed, it is a button in the corner."))
            .arg(ArgType::bool("minimized", false).description("Only its title bar."))
            .arg(ArgType::number("width", 400.0, 240.0, 640.0, 10.0))
            .arg(ArgType::number("height", 520.0, 200.0, 800.0, 10.0))
            .story("Open", Args::new())
            .story("Closed", Args::new().with("open", false))
            .story("Minimized", Args::new().with("minimized", true))
    }
}
