//! The stories of `Image`, from the component gallery's example, by scripts/rcb-scaffold.py.
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

/// The gallery's example of `Image`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_corner_radius = args.number("corner_radius") as f32;
        let arg_crossfade = args.number("crossfade");
        let arg_alt = args.text("alt");
        let arg_content_mode = match args.choice("content_mode") {
            "Fit" => ContentMode::Fit,
            "Fill" => ContentMode::Fill,
            _ => ContentMode::Cover,
        };
        let arg_opacity = args.number("opacity") as f32;
        let arg_stretch = args.bool("stretch");

        HStack::new((
            // `.crossfade` dissolves into the next image the app hands it.
            {
                let mut it = Image::new(picture(1.0)).size(160.0, 90.0);
                it = it.corner_radius(arg_corner_radius);
                it = it.crossfade(arg_crossfade);
                if !arg_alt.is_empty() {
                    it = it.alt(arg_alt.clone());
                }
                it = it.content_mode(arg_content_mode.clone());
                it = it.opacity(arg_opacity);
                if arg_stretch {
                    it = it.stretch();
                }
                it
            },
            Image::new(picture(0.3)).size(90.0, 90.0).corner_radius(45.0),
        ))
        .spacing(12.0)
    })
}

impl ComponentBook for Image {
    fn book() -> Meta {
        Meta::new("Media/Image", render)
            .summary("Shows an image, with fit modes.")
            .arg(ArgType::number("corner_radius", 12.0, 0.0, 48.0, 0.1))
            .arg(ArgType::number("crossfade", 0.2, 0.0, 1.0, 0.01))
            .arg(ArgType::text("alt", "").description("Empty leaves it as the component has it."))
            .arg(ArgType::choice("content_mode", &["Cover", "Fit", "Fill"], "Cover"))
            .arg(ArgType::number("opacity", 1.0, 0.0, 1.0, 0.05))
            .arg(ArgType::bool("stretch", false))
            .story("Example", Args::new())
            .story("Square corners", Args::new().with("corner_radius", 0.0))
            .story("Content mode: Fit", Args::new().with("content_mode", ArgValue::choice("Fit")))
            .story("Content mode: Fill", Args::new().with("content_mode", ArgValue::choice("Fill")))
    }
}
