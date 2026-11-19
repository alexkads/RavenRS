//! The stories of `CompareView`. Written by hand: the gallery's example returns early when its
//! pictures cannot be made, which the scaffold does not take apart.

use ravenrs_core::{Color, ImageData};
use ravenrs_widgets_base::book::{ArgType, ArgValue, Args, ComponentBook, Meta};
use ravenrs_widgets_base::{AnyView, Text};

use crate::imaging::{CompareMode, CompareView};

/// A gradient of grey, warmer by `warm`, as the gallery draws one.
fn tone(warm: f32) -> Option<ImageData> {
    ImageData::from_fn(240, 160, move |x, y| {
        let light = 0.25 + 0.5 * x as f32 / 240.0;
        Color::rgb(light + warm, light, light - warm + 0.2 * y as f32 / 160.0)
    })
}

fn render(args: &Args) -> AnyView {
    let (Some(before), Some(after)) = (tone(0.0), tone(0.2)) else {
        return Box::new(Text::new("No pictures"));
    };
    let mode =
        if args.choice("mode") == "SideBySide" { CompareMode::SideBySide } else { CompareMode::Divider };
    let mut view = CompareView::new(before, after)
        .mode(mode)
        .size(args.number("width") as f32, args.number("height") as f32);
    if !args.bool("captions") {
        view = view.no_captions();
    }
    Box::new(view)
}

impl ComponentBook for CompareView {
    fn book() -> Meta {
        Meta::new("Media/CompareView", render)
            .summary("Two pictures side by side or across a divider you drag.")
            .arg(ArgType::choice("mode", &["Divider", "SideBySide"], "Divider"))
            .arg(ArgType::number("width", 420.0, 160.0, 760.0, 10.0))
            .arg(ArgType::number("height", 200.0, 80.0, 480.0, 10.0))
            .arg(ArgType::bool("captions", true).description("Before and After over each picture."))
            .story("Divider", Args::new())
            .story("Side by side", Args::new().with("mode", ArgValue::choice("SideBySide")))
            .story("No captions", Args::new().with("captions", false))
    }
}
