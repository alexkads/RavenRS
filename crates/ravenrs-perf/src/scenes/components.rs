//! The components that carry the most weight, each at its worst realistic case.

use ravenrs_core::{Event, ImageData, Point, PointerButton, Size};
use ravenrs_layout::Align;
use ravenrs_reactive::State;
use ravenrs_widgets::grid::{GridColumn, GridData, Summary, Value};
use ravenrs_widgets::*;

use crate::{
    Ceiling, Class, PerfScene, SCALE, Setup, Step, VIEWPORT, ceiling, hover_between, hover_buttons, nothing,
    resize, scroll_at,
};

fn common(interaction: &str, a: Point, b: Point) -> Step {
    match interaction {
        "hover" => hover_between(a, b),
        "scroll" => scroll_at(Point::new(640.0, 400.0)),
        "resize" => resize(),
        _ => nothing(),
    }
}

fn data_grid(interaction: &str) -> Setup {
    let rows = (0..100_000)
        .map(|i| vec![Value::from(format!("Customer {i:06}")), Value::from("Brazil"), Value::from(i % 500)])
        .collect();
    let grid = DataGrid::new(
        GridData::new(rows),
        [
            GridColumn::text("Customer"),
            GridColumn::text("Country"),
            GridColumn::number("Seats").summary(Summary::Sum),
        ],
    )
    .height(720.0);
    Setup {
        root: Box::new(grid),
        step: common(interaction, Point::new(300.0, 200.0), Point::new(300.0, 300.0)),
    }
}

fn scheduler(interaction: &str) -> Setup {
    let Some(start) = Date::new(2026, 11, 1) else { panic!("a valid date") };
    let axis = State::new(TimeAxis::days(start, 30));
    let resources: Vec<Resource> = (0..200).map(|i| Resource::new(i, format!("Resource {i}"))).collect();
    let events: Vec<SchedulerEvent> = (0..5_000u64)
        .map(|i| {
            let from = TimePoint::from_date(start).plus_days((i % 28) as i64);
            SchedulerEvent::new(i, i % 200, from, from.plus_days(1), format!("Event {i}"))
        })
        .collect();
    let view = Scheduler::new(axis.binding()).resources(resources).events(events);
    Setup {
        root: Box::new(view),
        step: common(interaction, Point::new(500.0, 200.0), Point::new(700.0, 300.0)),
    }
}

fn chart_values(shift: u32) -> Vec<f64> {
    (0..5000).map(|i| f64::from((i + shift) % 97) + f64::from(i) * 0.02).collect()
}

/// The window dragged narrower, a new width each frame, as nothing kept can answer for.
fn chart_drag() -> Step {
    Box::new(|ui, round| {
        let width = VIEWPORT.width - (round % 400) as f32;
        ui.set_viewport(Size::new(width, VIEWPORT.height), SCALE);
        true
    })
}

fn chart_step(interaction: &str, shift: State<u32>) -> Step {
    match interaction {
        "data" => Box::new(move |_, round| {
            shift.set(round);
            false
        }),
        "resize" => chart_drag(),
        _ => common(interaction, Point::new(300.0, 200.0), Point::new(700.0, 300.0)),
    }
}

/// A line with many more points than the screen has columns.
fn chart(interaction: &str) -> Setup {
    let shift = State::new(0u32);
    let data = shift.clone();
    let root = view_fn(move || {
        Chart::new(ChartKind::Line)
            .categories((0..5000).map(|i| i.to_string()))
            .series("Values", chart_values(data.get()))
            .height(600.0)
    });
    Setup { root: Box::new(root), step: chart_step(interaction, shift) }
}

/// A year of three series, in bars, filling the window.
fn chart_bars(interaction: &str) -> Setup {
    let shift = State::new(0u32);
    let data = shift.clone();
    let root = view_fn(move || {
        let at = data.get();
        let values = |by: u32| (0..12).map(move |i: u32| f64::from((i * 7 + by + at) % 23) + 2.0);
        Chart::new(ChartKind::Bar)
            .categories((1..=12).map(|i| format!("M{i}")))
            .series("First", values(0))
            .series("Second", values(5))
            .series("Third", values(11))
            .show_value_axis()
            .height(600.0)
    });
    Setup { root: Box::new(root), step: chart_step(interaction, shift) }
}

/// The charts of a dashboard, side by side, about as big as they get on a real one.
fn chart_board(interaction: &str) -> Setup {
    let shift = State::new(0u32);
    let data = shift.clone();
    let root = view_fn(move || {
        let at = data.get();
        let values = |by: u32| (0..24).map(move |i: u32| f64::from((i * 7 + by + at) % 23) + 2.0);
        let months = || (1..=24).map(|i| format!("M{i}"));
        let area = Chart::new(ChartKind::Area)
            .categories(months())
            .series("Balance", values(0))
            .curve(ChartCurve::Monotone)
            .gradient()
            .height(380.0);
        let bars = Chart::new(ChartKind::Bar)
            .categories(months())
            .series("In", values(3))
            .series("Out", values(9))
            .height(380.0);
        let ring = Chart::new(ChartKind::Doughnut)
            .categories((1..=6).map(|i| format!("S{i}")))
            .series("Spent", (1..=6).map(f64::from))
            .height(380.0);
        let line = Chart::new(ChartKind::Line)
            .categories(months())
            .series("A", values(1))
            .series("B", values(13))
            .dots()
            .height(380.0);
        VStack::new((
            HStack::new((area.grow(1.0), bars.grow(1.0))).align(Align::Stretch),
            HStack::new((ring.grow(1.0), line.grow(1.0))).align(Align::Stretch),
        ))
        .align(Align::Stretch)
    });
    Setup { root: Box::new(root), step: chart_step(interaction, shift) }
}

/// A dozen flows between ten nodes, filling the window.
fn chart_sankey(interaction: &str) -> Setup {
    let shift = State::new(0u32);
    let data = shift.clone();
    let root = view_fn(move || {
        let at = f64::from(data.get() % 7);
        SankeyChart::new()
            .flow("Salary", "Taxes", 1200.0 + at)
            .flow("Salary", "Home", 1500.0)
            .flow("Salary", "Savings", 800.0)
            .flow("Salary", "Fun", 500.0)
            .flow("Freelance", "Savings", 600.0)
            .flow("Freelance", "Fun", 200.0)
            .flow("Home", "Rent", 1100.0)
            .flow("Home", "Power", 400.0)
            .flow("Savings", "Funds", 900.0)
            .flow("Savings", "Stocks", 500.0)
            .height(600.0)
    });
    Setup { root: Box::new(root), step: chart_step(interaction, shift) }
}

/// A square that grows, in the file format of the Lottie player.
const LOTTIE: &str = r#"{"v":"5.7.0","fr":30,"ip":0,"op":60,"w":100,"h":100,"layers":[{"ty":4,"ks":{},
"ip":0,"op":60,"st":0,"shapes":[{"ty":"rc","p":{"a":0,"k":[50,50]},"s":{"a":1,"k":[
{"t":0,"s":[10,100],"o":{"x":[0],"y":[0]},"i":{"x":[1],"y":[1]}},{"t":60,"s":[100,100]}]},"r":{"a":0,"k":0}},
{"ty":"fl","c":{"a":0,"k":[1,0,0,1]},"o":{"a":0,"k":100}}]}]}"#;

fn lottie(interaction: &str) -> Setup {
    let Ok(animation) = LottieAnimation::from_json(LOTTIE.as_bytes()) else { panic!("a Lottie") };
    let filler: Vec<AnyView> =
        (0..40).map(|i| -> AnyView { Box::new(Card::new(Text::new(format!("Row {i}")))) }).collect();
    let root =
        VStack::new((Lottie::new(animation).size(96.0, 96.0), ScrollView::new(filler).grow(1.0))).grow(1.0);
    Setup {
        root: Box::new(root),
        step: common(interaction, Point::new(300.0, 200.0), Point::new(700.0, 300.0)),
    }
}

fn images(interaction: &str) -> Setup {
    let pictures: Vec<AnyView> = (0..600u32)
        .map(|i| -> AnyView {
            let pixels: Vec<u8> = (0..32u32 * 32).flat_map(|p| [(i % 256) as u8, p as u8, 90, 255]).collect();
            let Some(image) = ImageData::from_rgba8(32, 32, pixels) else { panic!("a matching buffer") };
            Box::new(Image::new(image).stretch().height(48.0))
        })
        .collect();
    let grid: AnyView = Box::new(Grid::new(12, pictures).spacing(8.0));
    Setup {
        root: Box::new(ScrollView::new(vec![grid]).grow(1.0)),
        step: common(interaction, Point::new(300.0, 200.0), Point::new(700.0, 300.0)),
    }
}

const VIRTUALIZED: &[Ceiling] =
    &[ceiling("*", "instances", 1000), ceiling("scroll", "shapes", 100), ceiling("idle", "frames", 0)];
const ANIMATED: &[Ceiling] = &[ceiling("animate", "builds", 0), ceiling("animate", "shapes", 0)];
const STILL: &[Ceiling] = &[ceiling("idle", "frames", 0)];
/// Twenty buttons and a number: a click must stay this small.
const CALCULATOR: &[Ceiling] = &[
    ceiling("*", "instances", 60),
    ceiling("*", "draw_calls", 60),
    ceiling("press", "builds", 1),
    ceiling("press", "shapes", 0),
    ceiling("idle", "frames", 0),
];

/// Clicks two buttons in turn, as a person using the screen does.
fn clicking(first: &'static str, second: &'static str) -> Step {
    Box::new(move |ui, round| {
        let name = if round % 2 == 0 { first } else { second };
        let tree = ui.access_tree();
        let found = tree.nodes.iter().find(|node| node.name.as_deref() == Some(name));
        let Some(position) = found.map(|node| node.bounds.center()) else {
            panic!("no button {name} on screen");
        };
        ui.handle_event(&Event::PointerMove { position });
        ui.handle_event(&Event::PointerDown { position, button: PointerButton::Primary });
        ui.handle_event(&Event::PointerUp { position, button: PointerButton::Primary })
    })
}

/// The calculator example, as it is: a small screen where a click changes one number.
fn calculator(interaction: &str) -> Setup {
    let step: Step = match interaction {
        "hover" => hover_buttons(),
        "press" => clicking("7", "Clear"),
        "resize" => resize(),
        _ => nothing(),
    };
    Setup { root: Box::new(calculator::app()), step }
}

pub(super) fn scenes() -> Vec<PerfScene> {
    let scene = |name, covers, class, interactions, make, ceilings| PerfScene {
        name,
        covers,
        class,
        interactions,
        make,
        ceilings,
    };
    let lists = &["hover", "scroll", "idle", "resize"][..];
    const CHART: &[&str] = &["hover", "data", "idle", "resize"];
    vec![
        scene(
            "data-grid",
            &["DataGrid"][..],
            Class::Virtualized,
            lists,
            data_grid as fn(&str) -> Setup,
            VIRTUALIZED,
        ),
        scene("scheduler", &["Scheduler"], Class::Virtualized, lists, scheduler, VIRTUALIZED),
        scene("chart", &["Chart"], Class::Picture, CHART, chart, STILL),
        scene("chart-bars", &["Chart", "BarChart"], Class::Picture, CHART, chart_bars, STILL),
        scene("chart-board", &["Chart", "AreaChart"], Class::Picture, CHART, chart_board, STILL),
        scene("chart-sankey", &["SankeyChart"], Class::Static, CHART, chart_sankey, STILL),
        scene("lottie", &["Lottie"], Class::Animated, &["animate", "hover", "scroll"], lottie, ANIMATED),
        scene("images", &["Image", "Grid"], Class::Static, &["hover", "scroll", "idle"], images, STILL),
        scene(
            "calculator",
            &["Button", "Grid", "Text", "VStack"],
            Class::App,
            &["hover", "press", "idle", "resize"],
            calculator,
            CALCULATOR,
        ),
    ]
}
