//! The stories of `Ruler`, from the gallery's example. Reviewed: it is given a width, since a
//! ruler fills what it is given.

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

/// The gallery's example of `Ruler`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_scale = args.number("scale") as f32;
        let arg_origin = args.number("origin") as f32;
        let arg_marker = args.number("marker") as f32;
        let arg_whole_units = args.bool("whole_units");

        VStack::new((
            {
                let mut it = Ruler::horizontal();
                it = it.scale(arg_scale);
                it = it.origin(arg_origin);
                it = it.marker(arg_marker);
                if arg_whole_units {
                    it = it.whole_units();
                }
                it
            },
            Ruler::horizontal().scale(0.2).whole_units(),
        ))
        .spacing(8.0)
        .align(Align::Stretch)
        // A ruler takes the width it is given; the book gives it one, as a screen would.
        .width(480.0)
    })
}

impl ComponentBook for Ruler {
    fn book() -> Meta {
        Meta::new("Media/Ruler", render)
            .summary("Horizontal or vertical ruler in your units, with a round step for any zoom.")
            .arg(ArgType::number("scale", 2.0, 0.0, 8.0, 0.1))
            .arg(ArgType::number("origin", -20.0, -80.0, 80.0, 1.0))
            .arg(ArgType::number("marker", 60.0, 0.0, 240.0, 1.0))
            .arg(ArgType::bool("whole_units", false))
            .story("Example", Args::new())
    }
}
