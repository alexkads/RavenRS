//! The stories of `SplitPanes`, from the component gallery's example, by scripts/rcb-scaffold.py.
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

/// The gallery's example of `SplitPanes`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_height = args.number("height") as f32;
        let (collapsible, collapsed) = (args.bool("collapsible"), args.bool("collapsed"));

        {
            let mut sessions = SplitPane::new(Text::new("Sessions").padding(16.0))
                .size(180.0)
                .range(120.0, 260.0)
                .label("Sessions");
            let mut activity = SplitPane::new(Text::new("Activity").padding(16.0))
                .size(180.0)
                .range(120.0, 260.0)
                .label("Activity");
            if collapsible || collapsed {
                sessions = sessions.collapsible(true);
                activity = activity.collapsible(collapsible);
            }
            if collapsed {
                sessions = sessions.collapsed(State::new(true).binding());
            }
            let mut it = SplitPanes::horizontal()
                .pane(sessions)
                .pane(SplitPane::new(Text::new("Receipt").padding(16.0)).range(120.0, f32::MAX))
                .pane(activity);
            it = it.height(arg_height);
            it
        }
    })
}

impl ComponentBook for SplitPanes {
    fn book() -> Meta {
        Meta::new("Layout/SplitPanes", render)
            .summary("Any number of panes with a bar between each two, and limits.")
            .arg(ArgType::number("height", 190.0, 0.0, 760.0, 1.0))
            .arg(
                ArgType::bool("collapsible", false)
                    .description("Sessions and Activity fold to their edge with an arrow on the bar."),
            )
            .arg(
                ArgType::bool("collapsed", false)
                    .description("Sessions folded, a strip with the arrow back."),
            )
            .story("Example", Args::new())
            .story("Collapsible sides", Args::new().with("collapsible", true))
            .story("Collapsed", Args::new().with("collapsed", true))
    }
}
