//! The stories of `ScrollScene`, from the component gallery's example, by scripts/rcb-scaffold.py.
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

/// The gallery's example of `ScrollScene`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_length = args.number("length") as f32;
        let arg_top = args.number("top") as f32;

        // Each step fades in, holds for its share of the scene and fades out.
        let step = |title: &'static str, text: &'static str, from: f32, to: f32| {
            let away = Effect::hidden().then(Effect::moved(0.0, 16.0));
            let card = Card::new(
                VStack::new((
                    Text::new(title).style(TextStyle::Title),
                    Text::new(text).foreground(Semantic::InkMuted),
                ))
                .spacing(8.0),
            );
            let link = ScrollLink::keyframes([
                (from - 0.08, away),
                (from + 0.02, Effect::IDENTITY),
                (to - 0.02, Effect::IDENTITY),
                (to + 0.08, away),
            ]);
            ScrollLinked::new(card, link)
        };
        let steps = ZStack::new((
            step("1. State", "A click changes a value.", -1.0, 0.33),
            step("2. Layout", "Only what read it moves.", 0.33, 0.66),
            step("3. Paint", "The GPU draws the frame.", 0.66, 2.0),
        ));
        ScrollView::new((
            Text::new("Scroll down: the scene holds while its steps change.").foreground(Semantic::InkMuted),
            {
                let mut it = ScrollScene::new(steps);
                it = it.length(arg_length);
                it = it.top(arg_top);
                it
            },
            Text::new("Here the scene lets go.").foreground(Semantic::InkMuted),
            Canvas::new(|_, _| {}).size(10.0, 200.0),
        ))
        .spacing(12.0)
        .height(280.0)
    })
}

impl ComponentBook for ScrollScene<AnyView> {
    fn book() -> Meta {
        Meta::new("Layout/ScrollScene", render)
            .summary("A section that holds its content on screen for a length of scrolling, for steps that change as you scroll.")
            .arg(ArgType::number("length", 600.0, 0.0, 2400.0, 1.0))
            .arg(ArgType::number("top", 0.0, 0.0, 200.0, 1.0).description("Distance from the top of the scroll container while pinned, in points."))
            .story("Example", Args::new())
    }
}
