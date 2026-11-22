//! The stories of `Eyedropper`, from the component gallery's example, by scripts/rcb-scaffold.py.
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

/// The gallery's example of `Eyedropper`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_disabled = args.bool("disabled");
        let arg_hint = args.text("hint");
        let arg_label = args.text("label");
        let arg_size = match args.choice("size") {
            "Medium" => ButtonSize::Medium,
            "Large" => ButtonSize::Large,
            "Mini" => ButtonSize::Mini,
            _ => ButtonSize::Small,
        };

        let shown = State::new(Color::rgb(0.5, 0.5, 0.5));
        let told = shown.clone();
        let dropper = EyedropperState::new().on_pick(move |color| told.set(color));
        let ramp = |x: f32, y: f32| Color::rgb(x / 300.0, y / 130.0, 0.5);
        let picture = Canvas::new(move |frame, scene| {
            for i in 0..30 {
                let x = i as f32 * 10.0;
                let band = Rect::new(frame.x() + x, frame.y(), 10.0, frame.height());
                scene.push_quad(Quad::new(band, ramp(x, 65.0)));
            }
        })
        .size(300.0, 130.0);
        let swatch = Canvas::new(move |frame, scene| {
            scene.push_quad(Quad::new(frame, shown.get()).radius(4.0));
        })
        .size(28.0, 28.0);
        VStack::new((
            HStack::new((
                {
                    let mut it = Eyedropper::new(&dropper);
                    it = it.disabled(arg_disabled);
                    if !arg_hint.is_empty() {
                        it = it.hint(arg_hint.clone());
                    }
                    if !arg_label.is_empty() {
                        it = it.label(arg_label.clone());
                    }
                    it = it.size(arg_size.clone());
                    it
                },
                swatch,
            ))
            .spacing(8.0),
            EyedropperArea::new(&dropper, move |at| Some(ramp(at.x, at.y))).stage(picture),
        ))
        .spacing(8.0)
    })
}

impl ComponentBook for Eyedropper {
    fn book() -> Meta {
        Meta::new("Media/Eyedropper", render)
            .summary("An armed button that takes the color of the point clicked, supplied by the caller.")
            .arg(ArgType::bool("disabled", false))
            .arg(ArgType::text("hint", "").description("Empty leaves it as the component has it."))
            .arg(ArgType::text("label", "").description("Empty leaves it as the component has it."))
            .arg(ArgType::choice("size", &["Small", "Medium", "Large", "Mini"], "Small"))
            .story("Example", Args::new())
            .story("Disabled", Args::new().with("disabled", true))
            .story("Size: Medium", Args::new().with("size", ArgValue::choice("Medium")))
            .story("Size: Large", Args::new().with("size", ArgValue::choice("Large")))
            .story("Size: Mini", Args::new().with("size", ArgValue::choice("Mini")))
    }
}
