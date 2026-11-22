//! The stories of `ScrollView`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #760: it is no longer written over.

#[allow(unused_imports)]
use crate::book::{ArgType, ArgValue, Args, ComponentBook, Meta};
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
use std::collections::{BTreeMap, BTreeSet, HashMap};
#[allow(unused_imports)]
use std::rc::Rc;

/// The gallery's example of `ScrollView`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_spacing = args.number("spacing") as f32;
        let arg_height = args.number("height") as f32;
        let arg_horizontal = args.bool("horizontal");
        let arg_scrollbar_outside = args.number("scrollbar_outside") as f32;

        let rows: Vec<AnyView> =
            (1..=30).map(|n| Box::new(Text::new(format!("Row {n}"))) as AnyView).collect();
        {
            let mut it = ScrollView::new(rows);
            it = it.spacing(arg_spacing);
            it = it.height(arg_height);
            if arg_horizontal {
                it = it.horizontal();
            }
            it = it.scrollbar_outside(arg_scrollbar_outside);
            it
        }
    })
}

impl ComponentBook for ScrollView<(AnyView,)> {
    fn book() -> Meta {
        Meta::new("Layout/ScrollView", render)
            .summary("Scrolling stack, vertical or horizontal.")
            .arg(ArgType::number("spacing", 8.0, 0.0, 32.0, 0.1))
            .arg(ArgType::number("height", 180.0, 0.0, 720.0, 1.0))
            .arg(ArgType::bool("horizontal", false))
            .arg(
                ArgType::number("scrollbar_outside", 0.0, 0.0, 48.0, 1.0)
                    .description("How far past the right edge the scrollbar sits, in points."),
            )
            .story("Example", Args::new())
            .story("Horizontal", Args::new().with("horizontal", true))
    }
}
