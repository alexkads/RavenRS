//! The stories of `FloatingToolbar`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #755: it is no longer written over.

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

/// The gallery's example of `FloatingToolbar`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_label = args.text("label");
        let arg_separator = args.bool("separator");
        let arg_fill = args.bool("fill");

        let (place, level) = (State::new(None::<Point>), State::new(100.0_f32));
        {
            let mut it = FloatingToolbar::new(
                Spacer::new().frame(None, Some(200.0)).background(Semantic::Control).corner_radius(8.0),
            )
            .movable(place.binding())
            .action(ToolbarAction::new("Fit"))
            .action(ToolbarAction::new("Fill"))
            .action(ToolbarAction::new("Zoom out").icon(IconName::Minus).icon_only())
            .view(
                ValueSlider::new(level.binding()).range(10.0, 400.0).step(10.0).unit("%").label("Zoom level"),
            )
            .action(ToolbarAction::new("Zoom in").icon(IconName::Plus).icon_only());
            it = it.label(arg_label.clone());
            if arg_separator {
                it = it.separator();
            }
            if arg_fill {
                it = it.fill();
            }
            it
        }
    })
}

impl ComponentBook for FloatingToolbar<AnyView> {
    fn book() -> Meta {
        Meta::new("Actions and inputs/FloatingToolbar", render)
            .summary("Translucent row of commands over its content, with a handle that moves it.")
            .arg(ArgType::text("label", "Zoom"))
            .arg(ArgType::bool("separator", true))
            .arg(ArgType::bool("fill", false))
            .story("Example", Args::new())
    }
}
