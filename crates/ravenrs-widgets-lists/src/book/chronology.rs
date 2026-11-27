//! The stories of `Chronology`: moments down a line, with badges and in every tone.

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

/// The gallery's example of `Chronology`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    let badges = args.bool("badges");
    let tone = match args.choice("tone") {
        "Accent" => Tone::Accent,
        "Success" => Tone::Success,
        "Warning" => Tone::Warning,
        "Danger" => Tone::Danger,
        _ => Tone::Neutral,
    };
    let item = |id: u64, when: &str, title: &str, detail: &str, badge: &str| {
        let mut item = ChronologyItem::new(id, when, title).detail(detail).tone(tone);
        if badges {
            item = item.badge(badge);
        }
        item
    };
    Box::new(
        VStack::new((Chronology::new(vec![
            item(1, "02/03/2024", "Batismo", "Na igreja central", "Feito"),
            item(2, "10/08/2025", "Membro ativo", "Pela assembleia", "Aprovado"),
            item(3, "14/11/2026", "Transferência pedida", "Para a igreja do bairro", "Em análise"),
        ])
        .label(args.text("label")),))
        .width(420.0),
    )
}

impl ComponentBook for Chronology {
    fn book() -> Meta {
        Meta::new("Lists and trees/Chronology", render)
            .summary("Moments down a line: a date, a mark joined to the next, a title, a detail and a badge.")
            .arg(ArgType::choice("tone", &["Neutral", "Accent", "Success", "Warning", "Danger"], "Accent"))
            .arg(ArgType::bool("badges", false).description("A badge beside each title."))
            .arg(ArgType::text("label", "Marcos"))
            .story("Example", Args::new())
            .story("With badges", Args::new().with("badges", true))
            .story("Tones", Args::new().with("tone", ArgValue::choice("Success")).with("badges", true))
            .story("Tone: Neutral", Args::new().with("tone", ArgValue::choice("Neutral")))
            .story("Tone: Warning", Args::new().with("tone", ArgValue::choice("Warning")))
            .story("Tone: Danger", Args::new().with("tone", ArgValue::choice("Danger")))
    }
}
