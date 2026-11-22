//! The stories of `Protractor`, from the component gallery's example, by scripts/rcb-scaffold.py.
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

/// The gallery's example of `Protractor`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_label = args.text("label");
        let arg_disabled = args.bool("disabled");
        let arg_step = args.number("step") as f32;
        let arg_width = args.number("width") as f32;

        let angle = State::new(7.5_f32);
        {
            let mut it = Protractor::new(angle.binding());
            it = it.label(arg_label.clone());
            it = it.disabled(arg_disabled);
            it = it.step(arg_step);
            it = it.width(arg_width);
            it
        }
    })
}

impl ComponentBook for Protractor {
    fn book() -> Meta {
        Meta::new("Media/Protractor", render)
            .summary("A band of ticks along an arc that sets an angle.")
            .arg(ArgType::text("label", "Straighten"))
            .arg(ArgType::bool("disabled", false))
            .arg(ArgType::number("step", 0.1, 0.0, 1.0, 0.01))
            .arg(ArgType::number("width", 260.0, 0.0, 1040.0, 1.0))
            .story("Example", Args::new())
            .story("Disabled", Args::new().with("disabled", true))
    }
}
