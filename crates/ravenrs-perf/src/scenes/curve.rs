//! The same screen at three sizes. It answers the question of the core: does the cost
//! of a small change follow what changed, or the size of the screen?

use ravenrs_core::Point;
use ravenrs_reactive::State;
use ravenrs_style::{Semantic, TextStyle};
use ravenrs_widgets::*;

use crate::{Ceiling, Class, PerfScene, Setup, Step, ceiling, hover_buttons, nothing, resize, scroll_at};

const INTERACTIONS: &[&str] = &["hover", "leaf_state", "scroll", "animate", "idle", "resize"];

fn screen(cards: usize, counter: State<u32>, animated: bool) -> AnyView {
    let rows: Vec<AnyView> = (0..cards)
        .map(|i| -> AnyView {
            Box::new(Card::new(HStack::new((
                VStack::new((
                    Text::new(format!("Setting {i}")).style(TextStyle::Headline),
                    Text::new("A short description of what this option changes.")
                        .foreground(Semantic::InkMuted),
                ))
                .grow(1.0),
                Badge::new("Beta"),
                Button::new("Configure").icon(IconName::Settings),
            ))))
        })
        .collect();
    let leaf = view_fn(move || Text::new(format!("Changed {} times", counter.get())));
    let list = ScrollView::new(rows).spacing(12.0).padding(24.0).grow(1.0);
    if animated {
        Box::new(VStack::new((Spinner::new(), leaf, list)).grow(1.0))
    } else {
        Box::new(VStack::new((leaf, list)).grow(1.0))
    }
}

fn make(cards: usize, interaction: &str) -> Setup {
    let counter = State::new(0);
    let step: Step = match interaction {
        "hover" => hover_buttons(),
        "leaf_state" => {
            let counter = counter.clone();
            Box::new(move |_, round| {
                counter.set(round);
                false
            })
        }
        "scroll" => scroll_at(Point::new(640.0, 400.0)),
        "resize" => resize(),
        _ => nothing(),
    };
    Setup { root: screen(cards, counter, interaction == "animate"), step }
}

/// What the screen may cost whatever its size: only what can be seen is drawn, an
/// animation or a scroll runs no view, and text already shaped is not shaped again.
const CEILINGS: &[Ceiling] = &[
    ceiling("*", "instances", 700),
    ceiling("*", "draw_calls", 80),
    ceiling("*", "uploaded_bytes", 40_000),
    ceiling("hover", "shapes", 0),
    ceiling("scroll", "builds", 0),
    ceiling("scroll", "shapes", 0),
    ceiling("animate", "builds", 0),
    ceiling("idle", "frames", 0),
];

const fn scene(name: &'static str, make: fn(&str) -> Setup) -> PerfScene {
    PerfScene {
        name,
        covers: &["Card", "HStack", "VStack", "Text", "Badge", "Button", "ScrollView", "Spinner"],
        class: Class::Static,
        interactions: INTERACTIONS,
        make,
        ceilings: CEILINGS,
    }
}

pub(super) fn scenes() -> Vec<PerfScene> {
    vec![
        scene("curve-small", |interaction| make(10, interaction)),
        scene("curve-medium", |interaction| make(100, interaction)),
        scene("curve-large", |interaction| make(1000, interaction)),
    ]
}
