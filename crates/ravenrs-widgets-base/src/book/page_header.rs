//! The stories of `PageHeader`, from the component gallery's example, by scripts/rcb-scaffold.py.
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

/// The gallery's example of `PageHeader`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_description = args.text("description");
        let arg_compact = args.bool("compact");

        {
            let mut it = PageHeader::new(args.text("title"));
            // The badge beside the title: a count, or a state with its icon (#921).
            if !args.text("badge").is_empty() {
                let mut badge = Badge::new(args.text("badge"));
                if let Some(icon) = super::button::icon(args.choice("badge_icon")) {
                    badge = badge.icon(icon);
                }
                it = it.badge(badge);
            }
            if args.bool("action") {
                it = it.action(Button::new("New session").icon(IconName::Plus));
            }
            if !arg_description.is_empty() {
                it = it.description(arg_description.clone());
            }
            if arg_compact {
                it = it.compact();
            }
            it
        }
    })
}

impl ComponentBook for PageHeader {
    fn book() -> Meta {
        Meta::new("Layout/PageHeader", render)
            .summary("Title, description, badges and actions at the top of a page.")
            .arg(ArgType::text("title", "Sessions"))
            .arg(ArgType::text("description", "Every shoot, newest first."))
            .arg(ArgType::bool("action", true).description("A button at the end of the line."))
            .arg(ArgType::bool("compact", false))
            .arg(ArgType::text("badge", "12").description("Beside the title; empty shows none."))
            .arg(ArgType::choice("badge_icon", super::button::ICONS, "none"))
            .story("Example", Args::new())
            .story("Compact", Args::new().with("compact", true))
            .story("No action", Args::new().with("action", false))
    }
}
