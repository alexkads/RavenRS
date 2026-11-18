//! The stories of `Tabs`.

use ravenrs_reactive::State;
use ravenrs_style::Semantic;
use ravenrs_widgets_base::book::{ArgType, ArgValue, Args, ComponentBook, Meta};
use ravenrs_widgets_base::{AnyView, Text};

use crate::{Tabs, TabsVariant};

fn tabs(args: &Args) -> AnyView {
    let tab = State::new(args.number("selected") as usize);
    let variant =
        if args.choice("variant") == "Segmented" { TabsVariant::Segmented } else { TabsVariant::Underline };
    let mut tabs = Tabs::new(tab.binding()).variant(variant);
    for name in args.text("tabs").split(',').map(str::trim).filter(|n| !n.is_empty()) {
        tabs = tabs.tab(name, Text::new(format!("The content of {name}.")).foreground(Semantic::InkMuted));
    }
    Box::new(tabs)
}

impl ComponentBook for Tabs {
    fn book() -> Meta {
        Meta::new("Navigation/Tabs", tabs)
            .summary("Switches between views of the same place; each tab builds when it is selected.")
            .arg(
                ArgType::text("tabs", "Activity, Settings, Billing")
                    .description("The tabs, separated by commas."),
            )
            .arg(ArgType::number("selected", 0.0, 0.0, 5.0, 1.0))
            .arg(ArgType::choice("variant", &["Underline", "Segmented"], "Underline"))
            .story("Underline", Args::new())
            .story(
                "Segmented",
                Args::new().with("variant", ArgValue::choice("Segmented")).with("selected", 1.0),
            )
    }
}
