//! The stories of `RichText`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #756: it is no longer written over.

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

/// The gallery's example of `RichText`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_links_only = args.bool("links_only");

        let opened = State::new(args.text("opened"));
        let told = opened.clone();
        VStack::new((
        {
        let mut it = RichText::new(
            "*Ready:* _your order_ ~left~ is at `Desk 4`.\n* Details at https://example.com/orders\n> Open until six",
        )
        .on_link(move |address| told.set(format!("Asked to open {address}")));
        if arg_links_only { it = it.links_only(); }
        it
    },
        Text::bound(&opened, Clone::clone).style(TextStyle::Caption).foreground(Semantic::InkMuted),
    ))
    .spacing(8.0)
    .align(Align::Stretch)
    .width(360.0)
    })
}

impl ComponentBook for RichText {
    fn book() -> Meta {
        Meta::new("Forms/RichText", render)
            .summary("Bold, italic, code and links in plain text, with the marks chat apps use.")
            .arg(ArgType::bool("links_only", false))
            .arg(ArgType::text("opened", "No link opened yet").description("How the example starts."))
            .story("Example", Args::new())
    }
}
