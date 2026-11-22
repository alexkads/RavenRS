//! The stories of `Carousel`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #762: it is no longer written over.

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

/// A gradient picture, as the component gallery draws one.
fn picture(hue: f32) -> ImageData {
    ImageData::from_fn(160, 90, move |x, y| {
        let (u, v) = (x as f32 / 160.0, y as f32 / 90.0);
        Color::rgb(0.2 + 0.6 * u * hue, 0.3 + 0.4 * v, 0.9 - 0.5 * u)
    })
    .unwrap_or_else(|| unreachable!("the size is not zero"))
}

/// The gallery's example of `Carousel`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_height = args.number("height") as f32;

        let slides: Vec<AnyView> = [1.0, 0.6, 0.2]
            .into_iter()
            .map(|hue| Box::new(Image::new(picture(hue)).stretch().height(220.0)) as AnyView)
            .collect();
        {
            let mut it = Carousel::new(slides);
            it = it.height(arg_height);
            it
        }
    })
}

impl ComponentBook for Carousel {
    fn book() -> Meta {
        Meta::new("Media/Carousel", render)
            .summary("Slides with buttons, position dots and optional autoplay.")
            .arg(ArgType::number("height", 220.0, 0.0, 880.0, 1.0))
            .story("Example", Args::new())
            .story("Short", Args::new().with("height", 140.0))
    }
}
