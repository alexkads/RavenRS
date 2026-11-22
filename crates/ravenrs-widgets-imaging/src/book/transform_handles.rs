//! The stories of `TransformHandles`, from the component gallery's example, by scripts/rcb-scaffold.py.
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

/// The gallery's example of `TransformHandles`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_disabled = args.bool("disabled");
        let arg_label = args.text("label");

        let layer = State::new(BoxTransform::new(Rect::new(110.0, 70.0, 180.0, 100.0)).rotated(-8.0));
        let shown = layer.clone();
        let picture = Canvas::new(move |frame, scene| {
            let p = shown.get();
            let at = p.rect.translate(frame.origin);
            // ravenrs-lint: allow(raw_color) the stand-in canvas behind the layer being transformed
            scene.push_quad(Quad::new(frame, Color::rgb(0.2, 0.27, 0.35)));
            // ravenrs-lint: allow(raw_color) the layer being transformed, as the user painted it
            scene.push_quad(Quad::new(at, Color::rgb(0.3, 0.55, 0.75).with_alpha(0.6)));
        })
        .size(420.0, 230.0);
        {
            let mut it = TransformHandles::new(layer.binding()).stage(picture);
            it = it.disabled(arg_disabled);
            if !arg_label.is_empty() {
                it = it.label(arg_label.clone());
            }
            it
        }
    })
}

impl ComponentBook for TransformHandles {
    fn book() -> Meta {
        Meta::new("Media/TransformHandles", render)
            .summary("Eight handles, a turn and a pivot over a box, returning its geometry.")
            .arg(ArgType::bool("disabled", false))
            .arg(ArgType::text("label", "").description("Empty leaves it as the component has it."))
            .story("Example", Args::new())
            .story("Disabled", Args::new().with("disabled", true))
    }
}
