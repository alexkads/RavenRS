//! The stories of `ImageLightbox`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #762: it is no longer written over.

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

/// The gallery's example of `ImageLightbox`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_label = args.text("label");
        let arg_wrap = args.bool("wrap");

        let open = State::new(None::<usize>);
        let opener = open.clone();
        let shades = [0.25_f32, 0.45, 0.65];
        let pictures = shades.into_iter().filter_map(|shade| {
            let image = ImageData::from_fn(320, 200, move |x, _| Color::rgb(shade, 0.3, x as f32 / 320.0))?;
            Some(LightboxItem::new(image).caption(format!("Shade {shade}")))
        });
        VStack::new((Button::new("Open pictures").on_click(move || opener.set(Some(0))), {
            let mut it = ImageLightbox::new(open.binding(), pictures);
            if !arg_label.is_empty() {
                it = it.label(arg_label.clone());
            }
            it = it.wrap(arg_wrap);
            it
        }))
    })
}

impl ComponentBook for ImageLightbox {
    fn book() -> Meta {
        Meta::new("Media/ImageLightbox", render)
            .summary("A picture over the whole app, with zoom, arrows, Esc, caption and counter.")
            .arg(ArgType::text("label", "").description("Empty leaves it as the component has it."))
            .arg(ArgType::bool("wrap", false))
            .story("Example", Args::new())
    }
}
