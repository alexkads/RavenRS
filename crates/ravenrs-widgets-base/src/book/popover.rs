//! The stories of `Popover`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #758: it is no longer written over.

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

/// The gallery's example of `Popover`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_min_width = args.number("min_width") as f32;
        let arg_padding = args.number("padding") as f32;
        let arg_surface = args.bool("surface");

        let open = State::new(args.bool("open"));
        let flip = open.clone();
        {
            let mut it = Popover::new(
                open.binding(),
                Button::new("Show details")
                    .style(ButtonStyle::Secondary)
                    .on_click(move || flip.update(|o| *o = !*o)),
                VStack::new((Text::new("Storage").style(TextStyle::Headline), ProgressBar::new(0.62)))
                    .spacing(8.0)
                    .width(220.0),
            );
            it = it.min_width(arg_min_width);
            it = it.padding(arg_padding);
            it = it.surface(arg_surface);
            it
        }
        // `Placement::Above` and the others work too; `.surface(false)` drops the panel.
        .placement(Placement::Below)
    })
}

impl ComponentBook for Popover<AnyView, AnyView> {
    fn book() -> Meta {
        Meta::new("Overlays/Popover", render)
            .summary("Floating content anchored below a view.")
            .arg(ArgType::number("min_width", 220.0, 0.0, 880.0, 1.0))
            .arg(ArgType::number("padding", 16.0, 0.0, 64.0, 1.0))
            .arg(ArgType::bool("surface", true))
            .arg(ArgType::bool("open", false).description("How the example starts."))
            .story("Example", Args::new())
            .story("Open", Args::new().with("open", true))
    }
}
