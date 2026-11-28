//! The stories of `Text`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #761: it is no longer written over. The whole type scale is a story of its own.

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

/// The gallery's example of `Text`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_truncate = args.bool("truncate");
        let arg_mono = args.bool("mono");
        let mono = |text: Text| if arg_mono { text.mono() } else { text };

        VStack::new((
            {
                let mut it = mono(Text::new("Display").style(TextStyle::Display));
                if arg_truncate {
                    it = it.truncate();
                }
                it
            },
            mono(Text::new("Headline").style(TextStyle::Headline)),
            mono(Text::new("Muted body text").foreground(Semantic::InkMuted)),
            mono(Text::new("In the middle").align(TextAlign::Center)),
            mono(Text::new("At the end").align(TextAlign::End)),
        ))
        .spacing(8.0)
        .align(Align::Stretch)
        .width(360.0)
    })
}

impl ComponentBook for Text {
    fn book() -> Meta {
        Meta::new("Display and feedback/Text", render)
            .summary("Text in the theme's type scale, with truncation.")
            .arg(ArgType::bool("truncate", false))
            .arg(
                ArgType::bool("mono", false)
                    .description("The monospaced family, Fira Mono, for code or the lines of a receipt."),
            )
            .story("Example", Args::new())
            .story("Truncated", Args::new().with("truncate", true))
            .story("Monospaced", Args::new().with("mono", true))
            .story_with("Type scale", Args::new(), type_scale)
    }
}

/// Every style of the type scale, largest first, each named by itself.
fn type_scale(_: &Args) -> AnyView {
    let styles = [
        ("Metric 42", TextStyle::Metric),
        ("Display 32", TextStyle::Display),
        ("Title 22", TextStyle::Title),
        ("Headline 17", TextStyle::Headline),
        ("Body 15", TextStyle::Body),
        ("Callout 13", TextStyle::Callout),
        ("Caption 12", TextStyle::Caption),
    ];
    let rows: Vec<AnyView> =
        styles.into_iter().map(|(name, style)| Box::new(Text::new(name).style(style)) as AnyView).collect();
    Box::new(VStack::new(rows).spacing(8.0).align(Align::Start))
}
