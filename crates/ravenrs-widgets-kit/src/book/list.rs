//! The stories of `List`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #759: it is no longer written over.

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
use std::collections::{BTreeMap, BTreeSet, HashMap};
#[allow(unused_imports)]
use std::rc::Rc;

/// The things a list of the book shows, in order.
const ITEMS: [(&str, &str, IconName, &str); 6] = [
    ("Checkout", "git · 8 s", IconName::CircleCheck, "8 s"),
    ("Lint", "cargo clippy", IconName::CircleCheck, "1 min 40 s"),
    ("Tests", "cargo nextest run", IconName::LoaderCircle, "4 min 24 s"),
    ("Image and push", "waiting", IconName::Package, ""),
    ("Status on the PR", "always, at the end", IconName::GitBranch, ""),
    ("Report.pdf", "2.4 MB", IconName::FileText, "Owner"),
];

/// `items` rows, each with its icon, its title, its second line when `subtitles` says, and the
/// words at its end (#921: a screen says how many rows it draws).
fn render(args: &Args) -> AnyView {
    let count = (args.number("items") as usize).clamp(1, ITEMS.len());
    let rows: Vec<AnyView> = ITEMS[..count]
        .iter()
        .map(|(title, line, icon, end)| -> AnyView {
            let mut item = ListItem::new(*title).icon(*icon).on_click(|| {});
            if args.bool("subtitles") {
                item = item.subtitle(*line);
            }
            if !end.is_empty() {
                item = item.trailing(*end);
            }
            Box::new(item)
        })
        .collect();
    Box::new(List::new(rows))
}

/// A person, with an avatar instead of an icon.
fn with_avatar(_: &Args) -> AnyView {
    Box::new(List::new((ListItem::new("Ana Lima")
        .subtitle("ana@northwind.dev")
        .avatar("Ana Lima")
        .trailing("Owner"),)))
}

impl ComponentBook for List<(AnyView,)> {
    fn book() -> Meta {
        Meta::new("Data/List", render)
            .summary("Bordered list with icons, titles and trailing text.")
            .arg(ArgType::number("items", 2.0, 1.0, 6.0, 1.0))
            .arg(ArgType::bool("subtitles", true).description("A second line under each title."))
            .story("Example", Args::new())
            .story("One line each", Args::new().with("items", 5.0).with("subtitles", false))
            .story_with("With an avatar", Args::new(), with_avatar)
    }
}
