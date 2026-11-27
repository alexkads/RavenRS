//! The stories of `Tabs`.

use ravenrs_reactive::State;
use ravenrs_style::Semantic;
use ravenrs_widgets_base::book::{ArgType, ArgValue, Args, ComponentBook, Meta};
use ravenrs_widgets_base::{AnyView, ButtonSize, Text, VStack};

use crate::{Tabs, TabsVariant};

fn size(args: &Args) -> ButtonSize {
    match args.choice("size") {
        "Small" => ButtonSize::Small,
        "Large" => ButtonSize::Large,
        _ => ButtonSize::Medium,
    }
}

/// The bar the settings describe, before any tab is added.
fn bar(args: &Args, tab: &State<usize>) -> Tabs {
    let variant =
        if args.choice("variant") == "Segmented" { TabsVariant::Segmented } else { TabsVariant::Underline };
    Tabs::new(tab.binding()).variant(variant).size(size(args)).disabled(args.bool("disabled"))
}

fn names(args: &Args) -> Vec<String> {
    args.text("tabs").split(',').map(str::trim).filter(|n| !n.is_empty()).map(String::from).collect()
}

fn tabs(args: &Args) -> AnyView {
    let tab = State::new(args.number("selected") as usize);
    let mut tabs = bar(args, &tab);
    for name in names(args) {
        let content = Text::new(format!("The content of {name}.")).foreground(Semantic::InkMuted);
        tabs = tabs.tab(name, content);
    }
    Box::new(tabs)
}

/// Tabs added with `item`: the bar alone, for an app that shows what goes with the open
/// tab somewhere else, such as a toolbar over a list.
fn bar_only(args: &Args) -> AnyView {
    let tab = State::new(args.number("selected") as usize);
    let mut tabs = bar(args, &tab);
    for name in names(args) {
        tabs = tabs.item(name);
    }
    Box::new(tabs)
}

/// The three sizes, one bar under the other.
fn all_sizes(args: &Args) -> AnyView {
    let bars: Vec<AnyView> = ["Small", "Medium", "Large"]
        .into_iter()
        .map(|chosen| bar_only(&args.clone().with("size", ArgValue::choice(chosen))))
        .collect();
    Box::new(VStack::new(bars).spacing(16.0))
}

/// Six channels in a column 360 wide, in both looks: the tabs that do not fit go to the
/// button at the end of the bar.
fn overflow(args: &Args) -> AnyView {
    let bars: Vec<AnyView> = ["Underline", "Segmented"]
        .into_iter()
        .map(|variant| bar_only(&args.clone().with("variant", ArgValue::choice(variant))))
        .collect();
    Box::new(VStack::new(bars).spacing(16.0).width(360.0))
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
            .arg(ArgType::choice("size", &["Small", "Medium", "Large"], "Medium"))
            .arg(ArgType::bool("disabled", false).description("The open tab cannot change."))
            .story("Underline", Args::new())
            .story(
                "Segmented",
                Args::new().with("variant", ArgValue::choice("Segmented")).with("selected", 1.0),
            )
            .story_with("All sizes", Args::new(), all_sizes)
            .story("Disabled", Args::new().with("disabled", true).with("selected", 1.0))
            .story_with("Tabs without content", Args::new(), bar_only)
            .story_with(
                "Overflow",
                Args::new()
                    .with("tabs", "All, WhatsApp, Instagram, Messenger, Telegram, Site")
                    .with("size", ArgValue::choice("Small")),
                overflow,
            )
    }
}
