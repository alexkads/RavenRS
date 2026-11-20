//! The stories of `Minimap`, from the component gallery's example, by scripts/rcb-scaffold.py.
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

/// The gallery's example of `Minimap`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_label = args.text("label");

        let view = State::new(ZoomView::at(1.0));
        let picture = |frame: ZoomPanPaint, scene: &mut Scene| {
            // ravenrs-lint: allow(raw_color) the stand-in photograph the minimap shows: its colors are the content
            scene.push_quad(Quad::new(frame.placed, Color::rgb(0.2, 0.45, 0.7)));
            let sun = Rect::new(
                frame.placed.x() + frame.placed.width() * 0.6,
                frame.placed.y() + frame.placed.height() * 0.2,
                80.0 * frame.scale,
                80.0 * frame.scale,
            );
            // ravenrs-lint: allow(raw_color) the stand-in photograph the minimap shows: its colors are the content
            scene.push_quad(Quad::new(sun, Color::rgb(0.95, 0.8, 0.3)).radius(40.0 * frame.scale));
        };
        let content = Size::new(800.0, 500.0);
        HStack::new((
            {
                let mut it = Minimap::custom(view.binding(), content, picture);
                if !arg_label.is_empty() {
                    it = it.label(arg_label.clone());
                }
                it
            },
            ZoomPanView::custom(view.binding(), content, picture).size(240.0, 150.0),
        ))
        .spacing(12.0)
    })
}

impl ComponentBook for Minimap {
    fn book() -> Meta {
        Meta::new("Media/Minimap", render)
            .summary("Small copy of the content with a draggable rectangle around what is in view.")
            .arg(ArgType::text("label", "").description("Empty leaves it as the component has it."))
            .story("Example", Args::new())
    }
}
