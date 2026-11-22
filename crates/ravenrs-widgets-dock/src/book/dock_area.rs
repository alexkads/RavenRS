//! The stories of `DockArea`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #760: it is no longer written over.

#[allow(unused_imports)]
use crate::*;
#[allow(unused_imports)]
use ravenrs_core::*;
#[allow(unused_imports)]
use ravenrs_layout::*;
#[allow(unused_imports)]
use ravenrs_reactive::*;
#[allow(unused_imports)]
use ravenrs_style::*;
#[allow(unused_imports)]
use ravenrs_widgets_base::book::{ArgType, ArgValue, Args, ComponentBook, Meta};
#[allow(unused_imports)]
use ravenrs_widgets_base::*;
#[allow(unused_imports)]
use ravenrs_widgets_kit::*;
#[allow(unused_imports)]
use std::collections::{BTreeMap, BTreeSet, HashMap};
#[allow(unused_imports)]
use std::rc::Rc;

/// The gallery's example of `DockArea`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_auto_hide = args.bool("auto_hide");
        let arg_floatable = args.bool("floatable");
        let arg_free_placement = args.bool("free_placement");
        let arg_label = args.text("label");
        let arg_movable = args.bool("movable");
        let arg_tab_bar = match args.choice("tab_bar") {
            "WhenSeveral" => DockTabBar::WhenSeveral,
            "Never" => DockTabBar::Never,
            _ => DockTabBar::Always,
        };

        let layout = State::new(
            DockLayout::new()
                .center(DockNode::tabs(["photo", "grid"]))
                .left(DockNode::tabs(["folders", "search"]), 220.0)
                .right(DockNode::vertical([DockNode::tabs(["layers"]), DockNode::tabs(["history"])]), 200.0)
                .bottom(DockNode::tabs(["log"]), 110.0)
                .top(DockNode::tabs(["filters"]), 64.0),
        );
        let panels = vec![
            DockPanel::new("photo", "Photo").icon(IconName::Camera).closable(false),
            DockPanel::new("grid", "Grid").unsaved(true),
            DockPanel::new("folders", "Folders").icon(IconName::Folder),
            DockPanel::new("search", "Search").icon(IconName::Search),
            DockPanel::new("layers", "Layers"),
            DockPanel::new("history", "History"),
            DockPanel::new("log", "Log"),
            DockPanel::new("filters", "Filters").icon(IconName::Filter),
        ];
        {
            let mut it = DockArea::new(layout.binding(), panels, |id| -> AnyView {
                Box::new(VStack::new((Text::new(format!("What {id} shows")),)).padding(12.0))
            });
            it = it.auto_hide(arg_auto_hide);
            it = it.floatable(arg_floatable);
            it = it.free_placement(arg_free_placement);
            if !arg_label.is_empty() {
                it = it.label(arg_label.clone());
            }
            it = it.movable(arg_movable);
            it = it.tab_bar(arg_tab_bar.clone());
            it
        }
        // Keep this text, and give it to DockLayout::parse the next time.
        .on_end(|layout| println!("{}", layout.to_text()))
        .height(420.0)
    })
}

impl ComponentBook for DockArea {
    fn book() -> Meta {
        Meta::new("Layout/DockArea", render)
            .summary("Panels in a tree of splits and tabs that move by a drag, by keys or by a menu, resize, collapse, close, and save their arrangement as text.")
            .arg(ArgType::bool("auto_hide", true))
            .arg(ArgType::bool("floatable", true))
            .arg(ArgType::bool("free_placement", false))
            .arg(ArgType::text("label", "").description("Empty leaves it as the component has it."))
            .arg(ArgType::bool("movable", true))
            .arg(ArgType::choice("tab_bar", &["Always", "WhenSeveral", "Never"], "Always"))
            .story("Example", Args::new())
            .story("Tab bar: WhenSeveral", Args::new().with("tab_bar", ArgValue::choice("WhenSeveral")))
            .story("Tab bar: Never", Args::new().with("tab_bar", ArgValue::choice("Never")))
    }
}
