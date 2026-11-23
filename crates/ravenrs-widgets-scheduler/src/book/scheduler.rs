//! The stories of `Scheduler`, from the component gallery's example, by scripts/rcb-scaffold.py.
//! Reviewed for #756: it is no longer written over. With #919, the scale of quarters of an hour
//! and a resource that takes nothing, as a build server shows its agents.

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

/// The gallery's example of `Scheduler`, with its settings taken from the story.
#[allow(unused_variables, unused_mut, clippy::redundant_clone, clippy::let_and_return, clippy::clone_on_copy)]
fn render(args: &Args) -> AnyView {
    Box::new({
        let arg_avatars = args.bool("avatars");
        let arg_card = args.bool("card");
        let arg_fill_rows = args.bool("fill_rows");
        let arg_read_only = args.bool("read_only");
        let arg_resource_width = args.number("resource_width") as f32;
        let arg_toolbar = args.bool("toolbar");
        let arg_unavailable = args.bool("unavailable");

        // The last two hours of a build server, a row per agent.
        if args.choice("axis") == "Quarter hours" {
            let day = Date::new(2026, 11, 22).unwrap_or_else(|| unreachable!("a real date"));
            let at = |hour: u32, minute: u32| TimePoint::at(day, hour, minute);
            let axis = State::new(TimeAxis::quarter_hours(at(14, 0), 2));
            let mut agents = vec![
                Resource::new(1, "oficina-1").subtitle("local"),
                Resource::new(2, "docker").subtitle("ephemeral"),
                Resource::new(3, "k8s").subtitle("pods, up to 4"),
            ];
            let mut remote = Resource::new(4, "vps-backup").subtitle("remote");
            if arg_unavailable {
                remote = remote.unavailable("Disconnected 3 h ago · its build resumed on oficina-1");
            }
            agents.push(remote);
            let mut it = Scheduler::new(axis.binding())
                .resources(agents)
                .events([
                    SchedulerEvent::new(1, 1, at(14, 5), at(14, 22), "site #56").tone(Tone::Success).done(true),
                    SchedulerEvent::new(2, 1, at(14, 48), at(15, 43), "ravenrs PR-803 #42 · running").tone(Tone::Accent),
                    SchedulerEvent::new(3, 2, at(14, 27), at(14, 48), "loja-web #118").tone(Tone::Success).done(true),
                    SchedulerEvent::new(4, 3, at(14, 0), at(14, 36), "api-pedidos #31 · failed").tone(Tone::Danger).done(true),
                    SchedulerEvent::new(5, 3, at(14, 40), at(14, 46), "#12").tone(Tone::Warning).done(true),
                ])
                .now(at(15, 43));
            it = it.avatars(arg_avatars);
            it = it.card(arg_card);
            it = it.fill_rows(arg_fill_rows);
            it = it.read_only(arg_read_only);
            it = it.resource_width(arg_resource_width);
            it = it.toolbar(arg_toolbar);
            return Box::new(it);
        }

        let first = Date::new(2026, 11, 1).unwrap_or_else(|| unreachable!("a real date"));
        let at = |day: i64| TimePoint::from_date(first).plus_days(day);
        let axis = State::new(TimeAxis::days(first, 30));
        {
            let mut it = Scheduler::new(axis.binding())
                .resources([
                    Resource::new(1, "Ana"),
                    Resource::new(2, "Bruno"),
                    if arg_unavailable { Resource::new(3, "Carla").unavailable("On leave") } else { Resource::new(3, "Carla") },
                ])
                .events([
                    SchedulerEvent::new(1, 1, at(1), at(5), "Vacation").tone(Tone::Success),
                    SchedulerEvent::new(2, 1, at(3), at(8), "Workshop"),
                    SchedulerEvent::new(3, 2, at(2), at(6), "On call").tone(Tone::Warning),
                ])
                .on_change(|change| println!("{change:?}"));
            it = it.avatars(arg_avatars);
            it = it.card(arg_card);
            it = it.fill_rows(arg_fill_rows);
            it = it.read_only(arg_read_only);
            it = it.resource_width(arg_resource_width);
            it = it.toolbar(arg_toolbar);
            it
        }
    })
}

impl ComponentBook for Scheduler {
    fn book() -> Meta {
        Meta::new("Forms/Scheduler", render)
            .summary("Timeline of resources: drag, resize and draw events, zoom the time axis.")
            .arg(ArgType::bool("avatars", false))
            .arg(ArgType::bool("card", false))
            .arg(ArgType::bool("fill_rows", false))
            .arg(ArgType::bool("read_only", false))
            .arg(ArgType::number("resource_width", 200.0, 0.0, 800.0, 1.0))
            .arg(ArgType::bool("toolbar", true))
            .arg(ArgType::choice("axis", &["Days", "Quarter hours"], "Days"))
            .arg(ArgType::bool("unavailable", false).description("A resource that takes nothing now."))
            .story("Example", Args::new())
            .story("Read only", Args::new().with("read_only", true))
            .story(
                "Quarter hours",
                Args::new()
                    .with("axis", ArgValue::choice("Quarter hours"))
                    .with("read_only", true)
                    .with("unavailable", true),
            )
    }
}
