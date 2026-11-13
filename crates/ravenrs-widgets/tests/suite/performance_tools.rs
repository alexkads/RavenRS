//! The diagnostic tools, each judged the only way that counts: a defect planted on
//! purpose has to be pointed out by the tool meant to find it.
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use std::rc::Rc;
use std::time::{Duration, Instant};

use ravenrs_core::{BuildCause, Event, FrameLog, FrameSample, FrameWork, PhaseTimes, Point, Role, Size};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::*;

fn running(root: impl View + 'static) -> Ui {
    let mut ui = Ui::new(root, Theme::raven_light());
    ui.set_viewport(Size::new(800.0, 600.0), 1.0);
    ui.set_time(0.0);
    for _ in 0..3 {
        ui.update();
    }
    ui
}

fn buttons(ui: &mut Ui) -> Vec<Point> {
    let tree = ui.access_tree();
    tree.nodes.iter().filter(|n| n.role == Role::Button).map(|n| n.bounds.center()).collect()
}

#[test]
fn a_changed_state_is_named_as_the_cause() {
    let data = State::new(0);
    let read = data.clone();
    let mut ui = running(view_fn(move || Text::new(read.get().to_string())));
    let before = ui.stats().causes.count(BuildCause::Data);
    data.set(1);
    ui.update();
    assert_eq!(ui.stats().causes.count(BuildCause::Data), before + 1);
}

#[test]
fn moving_the_pointer_is_named_as_the_cause() {
    let mut ui = running(VStack::new((Button::new("One"), Button::new("Two"))));
    let places = buttons(&mut ui);
    let before = ui.stats().causes.count(BuildCause::Hover);
    ui.handle_event(&Event::PointerMove { position: places[0] });
    ui.update();
    assert!(ui.stats().causes.count(BuildCause::Hover) > before);
}

#[test]
fn a_frame_reports_why_it_built_and_what_it_shaped() {
    let data = State::new(0);
    let read = data.clone();
    let mut ui = running(view_fn(move || Text::new(read.get().to_string())));
    ui.take_frame_work();
    data.set(1);
    ui.update();
    let work = ui.take_frame_work();
    assert_eq!(work.builds, 1);
    assert_eq!(work.cause, Some(BuildCause::Data));
    assert!(work.shapes > 0);
    assert_eq!(ui.take_frame_work().builds, 0, "taking it starts over");
}

/// A component that wastes time on purpose: the defect to be found.
struct Wasteful;

impl View for Wasteful {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let started = Instant::now();
        while started.elapsed() < Duration::from_millis(3) {
            std::hint::spin_loop();
        }
        cx.child(&Text::new("Slow"))
    }
}

#[test]
fn a_slow_component_tops_the_list() {
    let mut ui =
        Ui::new(VStack::new((Text::new("Fast"), Wasteful, Button::new("Also fast"))), Theme::raven_light());
    let started = Instant::now();
    ui.set_phase_clock(Some(Rc::new(move || started.elapsed().as_secs_f64())));
    ui.set_view_profiling(true);
    ui.set_viewport(Size::new(800.0, 600.0), 1.0);
    ui.update();
    // The first build also loads the fonts, which is not any view's fault.
    ui.set_viewport(Size::new(801.0, 600.0), 1.0);
    ui.update();
    let costs = ui.view_costs();
    assert_eq!(costs.first().map(|cost| cost.short_name()), Some("Wasteful"), "{costs:?}");
    assert!(costs[0].time >= Duration::from_millis(3));
    assert!(costs[0].time < ui.phase_times().build, "the views inside do not count as its own");
}

#[test]
fn measuring_views_stays_off_by_default() {
    let ui = running(Text::new("Page"));
    assert!(ui.view_costs().is_empty());
    assert_eq!(ui.phase_times(), PhaseTimes::default());
}

#[test]
fn the_capture_is_a_trace_perfetto_can_open() {
    let mut log = FrameLog::default();
    log.record(FrameSample {
        update: Duration::from_millis(3),
        render: Duration::from_millis(1),
        work: FrameWork {
            phases: PhaseTimes { build: Duration::from_millis(2), ..PhaseTimes::default() },
            ..FrameWork::default()
        },
        ..FrameSample::default()
    });
    let trace: serde_json::Value = serde_json::from_str(&log.trace_json()).expect("valid JSON");
    let events = trace["traceEvents"].as_array().expect("a list of events");
    let names: Vec<&str> = events.iter().filter_map(|e| e["name"].as_str()).collect();
    assert_eq!(names, ["build", "other update", "render"]);
    assert_eq!(events[0]["ph"], "X");
    assert_eq!(events[0]["dur"], 2000.0);
    assert_eq!(events[2]["ts"], 3000.0, "phases follow each other");
}

#[test]
fn the_pointer_over_what_does_not_care_rebuilds_nothing() {
    let mut ui = running(VStack::new((Text::new("Plain"), Text::new("Text"), Button::new("Press"))));
    let builds = ui.stats().builds;
    for y in [5.0, 25.0] {
        ui.handle_event(&Event::PointerMove { position: Point::new(4.0, y) });
        ui.update();
    }
    assert_eq!(ui.stats().builds, builds, "nothing asked about the pointer there");
    let button = buttons(&mut ui)[0];
    ui.handle_event(&Event::PointerMove { position: button });
    ui.update();
    assert_eq!(ui.stats().builds, builds + 1, "a button does care");
    ui.handle_event(&Event::PointerMove { position: Point::new(4.0, 5.0) });
    ui.update();
    assert_eq!(ui.stats().builds, builds + 2, "and it has to know the pointer left");
}
