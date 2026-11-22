//! The stories of `Filmstrip`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #762: it is no longer written over.

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

/// The gallery's example of `Filmstrip`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_mode = match args.choice("mode") {
            "None" => SelectionMode::None,
            "Single" => SelectionMode::Single,
            _ => SelectionMode::Multiple,
        };
        let arg_cell_width = args.number("cell_width") as f32;
        let arg_height = args.number("height") as f32;
        let arg_label = args.text("label");
        let arg_spacing = args.number("spacing") as f32;

        let selected = State::new(BTreeSet::from([2_usize]));
        {
            let mut it = Filmstrip::new(5_000, |cell: FilmstripCell| {
                // The cell belongs to the app: its picture, its stars, its badges.
                let shade = (cell.index % 10) as f32 / 10.0;
                let picture = ImageData::from_fn(64, 64, move |x, y| {
                    Color::rgb(0.2 + shade * 0.5, 0.3 + y as f32 / 200.0, 0.7 - x as f32 / 200.0)
                });
                let picture: AnyView = match picture {
                    Some(picture) => Box::new(Image::new(picture).height(72.0).stretch()),
                    None => Box::new(Spacer::new()),
                };
                VStack::new((picture, Rating::read_only((cell.index % 6) as u32).size(11.0)))
                    .spacing(4.0)
                    .align(Align::Center)
            })
            .cell_name(|index| format!("Photo {}", index + 1))
            .selection(selected.binding())
            .fade(Semantic::SurfaceElevated);
            it = it.mode(arg_mode.clone());
            it = it.cell_width(arg_cell_width);
            it = it.height(arg_height);
            if !arg_label.is_empty() {
                it = it.label(arg_label.clone());
            }
            it = it.spacing(arg_spacing);
            it
        }
    })
}

impl ComponentBook for Filmstrip {
    fn book() -> Meta {
        Meta::new("Media/Filmstrip", render)
            .summary("A strip of thumbnails of any length, whose cells come from the app.")
            .arg(ArgType::choice("mode", &["None", "Single", "Multiple"], "Multiple"))
            .arg(ArgType::number("cell_width", 120.0, 0.0, 480.0, 1.0))
            .arg(ArgType::number("height", 112.0, 0.0, 448.0, 1.0))
            .arg(ArgType::text("label", "").description("Empty leaves it as the component has it."))
            .arg(ArgType::number("spacing", 6.0, 0.0, 24.0, 0.1))
            .story("Example", Args::new())
            .story("Mode: None", Args::new().with("mode", ArgValue::choice("None")))
            .story("Mode: Single", Args::new().with("mode", ArgValue::choice("Single")))
    }
}
