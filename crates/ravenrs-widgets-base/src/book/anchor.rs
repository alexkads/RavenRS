//! The stories of `Anchor`, from the component gallery's example, by scripts/rcb-scaffold.py.
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

/// The gallery's example of `Anchor`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_offset = args.number("offset") as f32;

        let rows: Vec<AnyView> =
            (1..=12).map(|n| Box::new(Text::new(format!("Row {n}"))) as AnyView).collect();
        ScrollView::new((
            Link::new("Jump to the summary", "#summary"),
            VStack::new(rows).spacing(8.0),
            {
                let mut it = Anchor::new("summary", Text::new("Summary").style(TextStyle::Title));
                it = it.offset(arg_offset);
                it
            },
            Canvas::new(|_, _| {}).size(10.0, 160.0),
        ))
        .spacing(12.0)
        .height(200.0)
    })
}

impl ComponentBook for Anchor<AnyView> {
    fn book() -> Meta {
        Meta::new("Layout/Anchor", render)
            .summary("A place a #name link scrolls to, gliding there.")
            .arg(
                ArgType::number("offset", 0.0, 0.0, 200.0, 1.0)
                    .description("Lands this far below the top, under a pinned bar, in points."),
            )
            .story("Example", Args::new())
    }
}
