//! The stories of `SwatchGrid`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #755: it is no longer written over.

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

/// The gallery's example of `SwatchGrid`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_columns = args.number("columns") as usize;
        let arg_label = args.text("label");

        let (front, back) = (State::new(Color::hex(0xe11d48)), State::new(Color::hex(0x2563eb)));
        let (for_front, for_back, shown) = (front.clone(), back.clone(), front.clone());
        VStack::new((
            view_fn(move || {
                let (for_front, for_back) = (for_front.clone(), for_back.clone());
                {
                    let mut it = SwatchGrid::new([
                        (Color::hex(0xe11d48), "Rose"),
                        (Color::hex(0xf59e0b), "Amber"),
                        (Color::hex(0x16a34a), "Green"),
                        (Color::hex(0x2563eb), "Blue"),
                        (Color::hex(0x7c3aed), "Violet"),
                        (Color::hex(0x111827), "Ink"),
                    ])
                    .selected(Some(shown.get()))
                    .on_pick(move |color, held| {
                        // With Cmd or Ctrl, the color goes to the background.
                        if held.meta || held.ctrl { for_back.set(color) } else { for_front.set(color) }
                    });
                    it = it.columns(arg_columns);
                    if !arg_label.is_empty() {
                        it = it.label(arg_label.clone());
                    }
                    it
                }
            }),
            Text::new("Click: foreground. Cmd or Ctrl and click: background.").style(TextStyle::Caption),
        ))
        .spacing(8.0)
    })
}

impl ComponentBook for SwatchGrid {
    fn book() -> Meta {
        Meta::new("Actions and inputs/SwatchGrid", render)
            .summary("Grid of color swatches; the handler gets the keys held with the click.")
            .arg(ArgType::number("columns", 3.0, 0.0, 12.0, 1.0))
            .arg(ArgType::text("label", "").description("Empty leaves it as the component has it."))
            .story("Example", Args::new())
    }
}
