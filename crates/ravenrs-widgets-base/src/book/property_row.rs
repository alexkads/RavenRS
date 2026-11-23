//! The stories of `PropertyRow`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #760: it is no longer written over.

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

/// One row: the label and its value, as a panel of details lists them (#921).
fn render(args: &Args) -> AnyView {
    Box::new(PropertyRow::new(args.text("label"), args.text("value")))
}

/// A value that is a view: a badge that says a state.
fn with_badge(_: &Args) -> AnyView {
    Box::new(PropertyRow::with("Status", Badge::new("Edited").tone(Tone::Success)))
}

impl ComponentBook for PropertyRow {
    fn book() -> Meta {
        Meta::new("Layout/PropertyRow", render)
            .summary("A label on the left and its value on the right.")
            .arg(ArgType::text("label", "Camera"))
            .arg(ArgType::text("value", "NIKON D3100"))
            .story("Example", Args::new())
            .story("Empty value", Args::new().with("label", "Lens").with("value", ""))
            .story_with("With a badge", Args::new(), with_badge)
    }
}
