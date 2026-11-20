//! The stories of `ZoomControl`, from the component gallery's example, by scripts/rcb-scaffold.py.
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

/// The gallery's example of `ZoomControl`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_label = args.text("label");
        let arg_size = match args.choice("size") {
            "Medium" => ButtonSize::Medium,
            "Large" => ButtonSize::Large,
            "Mini" => ButtonSize::Mini,
            _ => ButtonSize::Small,
        };

        let view = State::new(ZoomView::at(1.5));
        {
            let mut it = ZoomControl::new(view.binding());
            if !arg_label.is_empty() {
                it = it.label(arg_label.clone());
            }
            it = it.size(arg_size.clone());
            it
        }
    })
}

impl ComponentBook for ZoomControl {
    fn book() -> Meta {
        Meta::new("Media/ZoomControl", render)
            .summary("Fit, 1:1, the percentage and buttons that zoom in and out.")
            .arg(ArgType::text("label", "").description("Empty leaves it as the component has it."))
            .arg(ArgType::choice("size", &["Small", "Medium", "Large", "Mini"], "Small"))
            .story("Example", Args::new())
            .story("Size: Medium", Args::new().with("size", ArgValue::choice("Medium")))
            .story("Size: Large", Args::new().with("size", ArgValue::choice("Large")))
            .story("Size: Mini", Args::new().with("size", ArgValue::choice("Mini")))
    }
}
