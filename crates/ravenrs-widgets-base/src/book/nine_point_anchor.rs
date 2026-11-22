//! The stories of `NinePointAnchor`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #755: it is no longer written over.

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

/// The gallery's example of `NinePointAnchor`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_cell_size = args.number("cell_size") as f32;
        let arg_label = args.text("label");

        let point = State::new(AnchorPoint::Center);
        {
            let mut it = NinePointAnchor::new(point.binding());
            it = it.cell_size(arg_cell_size);
            if !arg_label.is_empty() {
                it = it.label(arg_label.clone());
            }
            it
        }
    })
}

impl ComponentBook for NinePointAnchor {
    fn book() -> Meta {
        Meta::new("Actions and inputs/NinePointAnchor", render)
            .summary("Three by three grid for a point of reference, moved by the arrow keys.")
            .arg(ArgType::number("cell_size", 12.0, 0.0, 48.0, 0.1))
            .arg(ArgType::text("label", "").description("Empty leaves it as the component has it."))
            .story("Example", Args::new())
    }
}
