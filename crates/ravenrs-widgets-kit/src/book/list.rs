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
const ITEMS: [(&str, &str, IconName, &str, Tone); 6] = [
    ("Checkout", "git · 8 s", IconName::CircleCheck, "8 s", Tone::Success),
    ("Lint", "cargo clippy", IconName::CircleCheck, "1 min 40 s", Tone::Success),
    ("Tests", "cargo nextest run", IconName::LoaderCircle, "4 min 24 s", Tone::Accent),
    ("Image and push", "waiting", IconName::Package, "", Tone::Neutral),
    ("Status on the PR", "always, at the end", IconName::GitBranch, "", Tone::Neutral),
    ("Report.pdf", "2.4 MB", IconName::FileText, "Owner", Tone::Neutral),
];

/// `items` rows, each with its icon, its title, its second line when `subtitles` says, and the
/// words at its end (#921: a screen says how many rows it draws). `compact` packs them, `tones`
/// colors each icon by its state and `action` puts a button at the end of the first row.
fn render(args: &Args) -> AnyView {
    let count = (args.number("items") as usize).clamp(1, ITEMS.len());
    let rows: Vec<AnyView> = ITEMS[..count]
        .iter()
        .enumerate()
        .map(|(i, (title, line, icon, end, tone))| -> AnyView {
            let mut item = ListItem::new(*title).icon(*icon).on_click(|| {});
            if args.bool("subtitles") {
                item = item.subtitle(*line);
            }
            if !end.is_empty() {
                item = item.trailing(*end);
            }
            if args.bool("tones") {
                item = item.tone(*tone);
            }
            if args.bool("action") && i == 0 {
                item = item
                    .action(Button::new("Run again").style(ButtonStyle::Secondary).size(ButtonSize::Small));
            }
            Box::new(item)
        })
        .collect();
    let list = List::new(rows);
    Box::new(if args.bool("compact") { list.compact() } else { list })
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
            .arg(ArgType::bool("compact", false).description("Rows of 48, the subtitle beside the title."))
            .arg(ArgType::bool("tones", false).description("Each icon in the color of its state."))
            .arg(ArgType::bool("action", false).description("A button at the end of the first row."))
            .story("Example", Args::new())
            .story("One line each", Args::new().with("items", 5.0).with("subtitles", false))
            .story(
                "Compact",
                Args::new()
                    .with("items", 5.0)
                    .with("subtitles", false)
                    .with("compact", true)
                    .with("tones", true),
            )
            .story(
                "Compact queue",
                Args::new().with("items", 2.0).with("compact", true).with("tones", true).with("action", true),
            )
            .story_with("With an avatar", Args::new(), with_avatar)
    }
}
