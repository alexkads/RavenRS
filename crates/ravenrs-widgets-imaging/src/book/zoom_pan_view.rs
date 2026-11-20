//! The stories of `ZoomPanView`, from the component gallery's example, by scripts/rcb-scaffold.py.
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

/// The gallery's example of `ZoomPanView`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_label = args.text("label");
        let arg_grow = args.number("grow") as f32;
        let arg_inertia = args.bool("inertia");

        let view = State::new(ZoomView::fit());
        let content = Size::new(1200.0, 800.0);
        let picture = |frame: ZoomPanPaint, scene: &mut Scene| {
            // ravenrs-lint: allow(raw_color) the stand-in photograph being zoomed: its colors are the content
            scene.push_quad(Quad::new(frame.placed, Color::rgb(0.16, 0.3, 0.5)));
            let side = 120.0 * frame.scale;
            // ravenrs-lint: allow(raw_color) the stand-in photograph being zoomed: its colors are the content
            for (i, color) in [Color::rgb(0.9, 0.45, 0.3), Color::rgb(0.95, 0.8, 0.3)].into_iter().enumerate()
            {
                let x = frame.placed.x() + frame.placed.width() * (0.2 + 0.4 * i as f32);
                let y = frame.placed.y() + frame.placed.height() * 0.35;
                scene.push_quad(Quad::new(Rect::new(x, y, side, side), color).radius(side / 2.0));
            }
        };
        VStack::new((ZoomControl::new(view.binding()), {
            let mut it = ZoomPanView::custom(view.binding(), content, picture).size(420.0, 200.0);
            it = it.label(arg_label.clone());
            it = it.grow(arg_grow);
            it = it.inertia(arg_inertia);
            it
        }))
        .spacing(8.0)
    })
}

impl ComponentBook for ZoomPanView {
    fn book() -> Meta {
        Meta::new("Media/ZoomPanView", render)
            .summary("Zoom and pan with pinch, wheel, drag, keys and inertia, within limits.")
            .arg(ArgType::text("label", "Sample picture"))
            .arg(ArgType::number("grow", 1.0, 0.0, 4.0, 0.1))
            .arg(ArgType::bool("inertia", true))
            .story("Example", Args::new())
    }
}
