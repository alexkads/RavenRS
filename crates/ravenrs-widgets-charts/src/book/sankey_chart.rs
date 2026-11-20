//! The stories of `SankeyChart`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #759: it is no longer written over.

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

/// The gallery's example of `SankeyChart`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_title = args.text("title");
        let arg_height = args.number("height") as f32;
        let arg_alt = args.text("alt");
        let arg_hide_tooltip = args.bool("hide_tooltip");
        let arg_no_appear = args.bool("no_appear");
        let arg_no_gradient = args.bool("no_gradient");

        {
            let mut it = SankeyChart::new()
                .flow("Salary", "Taxes", 1200.0)
                .flow("Salary", "Home", 1500.0)
                .flow("Salary", "Savings", 800.0)
                .flow("Freelance", "Savings", 600.0)
                .flow("Home", "Rent", 1100.0)
                .flow("Home", "Power", 400.0)
                .flow("Savings", "Funds", 900.0)
                .flow("Savings", "Stocks", 500.0)
                .format(|value| format!("$ {value:.0}"));
            it = it.title(arg_title.clone());
            it = it.height(arg_height);
            if !arg_alt.is_empty() {
                it = it.alt(arg_alt.clone());
            }
            if arg_hide_tooltip {
                it = it.hide_tooltip();
            }
            if arg_no_appear {
                it = it.no_appear();
            }
            if arg_no_gradient {
                it = it.no_gradient();
            }
            it
        }
    })
}

impl ComponentBook for SankeyChart {
    fn book() -> Meta {
        Meta::new("Data/SankeyChart", render)
            .summary("How much flows from what to what: ribbons between nodes, arrow keys, and a clear message when the flows go in a circle.")
            .arg(ArgType::text("title", "Where the money goes"))
            .arg(ArgType::number("height", 300.0, 0.0, 1200.0, 1.0))
            .arg(ArgType::text("alt", "").description("Empty leaves it as the component has it."))
            .arg(ArgType::bool("hide_tooltip", false))
            .arg(ArgType::bool("no_appear", false))
            .arg(ArgType::bool("no_gradient", false))
            .story("Example", Args::new())
            .story("Flat ribbons", Args::new().with("no_gradient", true))
    }
}
