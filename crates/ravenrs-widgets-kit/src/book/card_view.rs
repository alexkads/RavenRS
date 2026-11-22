//! The stories of `CardView`, from the component gallery's example, by scripts/rcb-scaffold.py.
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

/// The gallery's example of `CardView`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_card_width = args.number("card_width") as f32;
        let arg_card_height = args.number("card_height") as f32;
        let arg_height = args.number("height") as f32;
        let arg_label = args.text("label");
        let arg_selection_mode = match args.choice("selection_mode") {
            "None" => SelectionMode::None,
            "Multiple" => SelectionMode::Multiple,
            _ => SelectionMode::Single,
        };

        let selected = State::new(std::collections::BTreeSet::from([1]));
        {
            let mut it = CardView::new(10_000, |i| -> AnyView {
                Box::new(
                    VStack::new((
                        Text::new(format!("Customer {i}")).font_weight(Weight::SEMIBOLD),
                        Text::new(format!("${}.00", 100 + i * 7)).foreground(Semantic::InkMuted),
                    ))
                    .spacing(4.0),
                )
            })
            .card_name(|i| format!("Customer {i}"))
            .selection(selected.binding());
            it = it.card_width(arg_card_width);
            it = it.card_height(arg_card_height);
            it = it.height(arg_height);
            if !arg_label.is_empty() {
                it = it.label(arg_label.clone());
            }
            it = it.selection_mode(arg_selection_mode.clone());
            it
        }
    })
}

impl ComponentBook for CardView {
    fn book() -> Meta {
        Meta::new("Data/CardView", render)
            .summary("Records as cards, only those in sight are built.")
            .arg(ArgType::number("card_width", 180.0, 0.0, 720.0, 1.0))
            .arg(ArgType::number("card_height", 84.0, 0.0, 336.0, 1.0))
            .arg(ArgType::number("height", 300.0, 0.0, 1200.0, 1.0))
            .arg(ArgType::text("label", "").description("Empty leaves it as the component has it."))
            .arg(ArgType::choice("selection_mode", &["None", "Single", "Multiple"], "Single"))
            .story("Example", Args::new())
            .story("Selection mode: None", Args::new().with("selection_mode", ArgValue::choice("None")))
            .story(
                "Selection mode: Multiple",
                Args::new().with("selection_mode", ArgValue::choice("Multiple")),
            )
    }
}
