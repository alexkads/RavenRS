//! The components that carry the most weight, each at its worst realistic case.

use ravenrs_core::{Color, Event, ImageData, Point, PointerButton, Rect, Size};
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

/// A drag that goes back and forth, from the first round on.
fn dragging(from: Point, to: Point) -> Step {
    Box::new(move |ui, round| {
        if round == 0 {
            ui.handle_event(&Event::PointerMove { position: from });
            return ui.handle_event(&Event::PointerDown { position: from, button: PointerButton::Primary });
        }
        let position = if round % 2 == 0 { from } else { to };
        ui.handle_event(&Event::PointerMove { position })
    })
}

/// A large picture alone on screen: a drag repaints it and runs no view.
fn zoom_pan(interaction: &str) -> Setup {
    let Some(picture) =
        ImageData::from_fn(1024, 1024, |x, y| Color::rgb(x as f32 / 1024.0, y as f32 / 1024.0, 0.4))
    else {
        panic!("a valid image")
    };
    let view = State::new(ZoomView::at(2.0));
    let middle = Point::new(600.0, 400.0);
    let step: Step = match interaction {
        "pan" => dragging(middle, Point::new(560.0, 370.0)),
        other => common(other, middle, Point::new(500.0, 300.0)),
    };
    Setup { root: Box::new(ZoomPanView::image(view.binding(), picture)), step }
}

/// The instruments together, around a stage that follows the same view: every drag
/// rebuilds this screen, which is what the counters answer for.
fn image_tools(interaction: &str) -> Setup {
    let Some(picture) =
        ImageData::from_fn(256, 256, |x, y| Color::rgb(x as f32 / 256.0, y as f32 / 256.0, 0.4))
    else {
        panic!("a valid image")
    };
    let hill = |center: f32| -> Vec<f32> {
        (0..256).map(|i| 500.0 * (-((i as f32 - center) / 40.0).powi(2)).exp() + 10.0).collect()
    };
    let channels = [
        HistogramChannel::new("Red", Color::rgb(0.9, 0.2, 0.2), hill(90.0)),
        HistogramChannel::new("Green", Color::rgb(0.2, 0.8, 0.2), hill(130.0)),
        HistogramChannel::new("Blue", Color::rgb(0.2, 0.3, 0.9), hill(170.0)),
    ];
    let view = State::new(ZoomView::at(1.0));
    let points =
        State::new(vec![Point::ZERO, Point::new(0.3, 0.2), Point::new(0.7, 0.8), Point::new(1.0, 1.0)]);
    let tint = State::new(HueSaturation::new(30.0, 0.5));
    let panel = VStack::new((
        Histogram::new(channels.clone()).height(120.0),
        CurveEditor::new(points.binding()).size(320.0, 220.0).histogram(channels, HistogramScale::Linear),
        ColorWheel::new(tint.binding()).diameter(180.0),
        ZoomControl::new(view.binding()),
    ))
    .spacing(8.0)
    .width(320.0);
    let stage = VStack::new((
        Ruler::horizontal().scale(view.get().scale).whole_units(),
        HStack::new((
            ZoomPanView::image(view.binding(), picture.clone()),
            Minimap::image(view.binding(), picture),
        ))
        .grow(1.0),
    ))
    .grow(1.0);
    let step: Step = match interaction {
        // The handle of the second point, and the disc.
        "curve" => dragging(Point::new(102.0, 287.0), Point::new(120.0, 260.0)),
        "wheel" => dragging(Point::new(130.0, 470.0), Point::new(90.0, 440.0)),
        other => common(other, Point::new(100.0, 60.0), Point::new(700.0, 300.0)),
    };
    Setup { root: Box::new(HStack::new((panel, stage)).grow(1.0)), step }
}

/// What is drawn over a picture, each dragged alone: nothing but the paint may run, except a
/// protractor, which writes its value as it goes. Idle and hover see them all together.
fn picture_overlays(interaction: &str) -> Setup {
    let picture = |seed: f32| {
        ImageData::from_fn(1024, 640, |x, y| Color::rgb(x as f32 / 1024.0, y as f32 / 640.0, seed))
    };
    let (Some(first), Some(second)) = (picture(0.3), picture(0.6)) else { panic!("valid images") };
    let area = Rect::new(200.0, 120.0, 500.0, 300.0);
    let stage = || Image::new(first.clone()).size(1024.0, 640.0);
    let (from, to) = (Point::new(300.0, 200.0), Point::new(360.0, 250.0));
    let selection = State::new(Some(area));
    let band = RubberBand::new(selection.binding());
    let held = State::new(BoxTransform::new(area).rotated(12.0));
    let handles = TransformHandles::new(held.binding());
    let cropped = State::new(CropArea::new(area).rotated(-6.0));
    let crop = CropOverlay::new(cropped.binding());
    let angle = State::new(5.0_f32);
    let dial = Protractor::new(angle.binding());
    let compare = CompareView::new(first.clone(), second.clone()).mode(CompareMode::Divider);
    let (root, step): (AnyView, Step) = match interaction {
        "band" => (Box::new(band.stage(stage())), dragging(from, to)),
        "handles" => (Box::new(handles.stage(stage())), dragging(from, to)),
        "crop" => (Box::new(crop.stage(stage())), dragging(from, to)),
        "divider" => (Box::new(compare), dragging(from, to)),
        "protractor" => (Box::new(dial), dragging(Point::new(60.0, 20.0), Point::new(200.0, 20.0))),
        other => {
            let all = ZStack::new((
                stage(),
                RuleOfThirdsGrid::new().size(1024.0, 640.0),
                Marquee::rect(area).animated(false).size(1024.0, 640.0),
                handles.size(1024.0, 640.0),
            ));
            (Box::new(VStack::new((all, dial)).spacing(8.0)), common(other, from, Point::new(900.0, 600.0)))
        }
    };
    Setup { root, step }
}

/// Every place of a dock area in use, with splits inside: a tab changes, and a bar is dragged.
fn docking(interaction: &str) -> Setup {
    const IDS: [(&str, &str); 8] = [
        ("photo", "Photo"),
        ("grid", "Grid"),
        ("props", "Properties"),
        ("folders", "Folders"),
        ("search", "Search"),
        ("layers", "Layers"),
        ("history", "History"),
        ("log", "Log"),
    ];
    let panels = IDS.iter().map(|(id, title)| DockPanel::new(*id, *title)).collect();
    let layout = State::new(
        DockLayout::new()
            .center(DockNode::horizontal([DockNode::tabs(["photo", "grid"]), DockNode::tabs(["props"])]))
            .left(DockNode::tabs(["folders", "search"]), 280.0)
            .right(DockNode::vertical([DockNode::tabs(["layers"]), DockNode::tabs(["history"])]), 280.0)
            .bottom(DockNode::tabs(["log"]), 160.0),
    );
    let area = DockArea::new(layout.binding(), panels, |id| -> AnyView {
        let lines: Vec<Text> = (0..12).map(|line| Text::new(format!("{id}, line {line}"))).collect();
        Box::new(VStack::new(lines).spacing(4.0))
    });
    // The bar of the left dock: past its strip and its 280 logical pixels.
    let (bar, further) = (Point::new(297.0, 300.0), Point::new(340.0, 300.0));
    let step: Step = match interaction {
        "tab" => clicking("Grid", "Photo"),
        "divider" => dragging(bar, further),
        other => common(other, Point::new(120.0, 16.0), Point::new(600.0, 16.0)),
    };
    Setup { root: Box::new(area), step }
}

/// Ten thousand rows of different heights, of which a screenful exists.
fn virtual_list(interaction: &str) -> Setup {
    let list = VirtualList::new(10_000, |index| {
        let lines = 1 + index % 3;
        let words = (0..lines).map(|line| format!("Row {index}, line {line}")).collect::<Vec<_>>().join("\n");
        Text::new(words)
    })
    .key(|index| index as u64)
    .estimated_height(40.0)
    .spacing(4.0)
    .height(720.0);
    Setup {
        root: Box::new(list),
        step: common(interaction, Point::new(300.0, 200.0), Point::new(300.0, 300.0)),
    }
}

/// Thousands of thumbnails of the same picture, each with its stars.
fn filmstrip(interaction: &str) -> Setup {
    let picture = ImageData::from_fn(96, 96, |x, y| Color::rgb(x as f32 / 96.0, y as f32 / 96.0, 0.4));
    let strip = Filmstrip::new(5_000, move |cell: FilmstripCell| {
        let thumbnail: AnyView = match &picture {
            Some(picture) => Box::new(Image::new(picture.clone()).size(112.0, 96.0)),
            None => Box::new(Spacer::new()),
        };
        VStack::new((thumbnail, Rating::read_only((cell.index % 6) as u32).size(12.0))).spacing(2.0)
    })
    .cell_width(120.0)
    .height(128.0);
    let (a, b) = (Point::new(200.0, 60.0), Point::new(460.0, 60.0));
    let step = match interaction {
        // The wheel over the strip itself: the screen has nothing else that scrolls.
        "scroll" => scroll_at(a),
        other => common(other, a, b),
    };
    Setup { root: Box::new(strip), step }
}

/// The dashes around a selection walk on their own, without a build.
fn marquee(_: &str) -> Setup {
    let root = Marquee::rect(Rect::new(100.0, 100.0, 800.0, 500.0));
    Setup { root: Box::new(root), step: nothing() }
}

/// A pan paints a textured rectangle: nothing else on it may run.
const ZOOM_PAN: &[Ceiling] = &[
    ceiling("*", "instances", 20),
    ceiling("pan", "builds", 0),
    ceiling("pan", "layouts", 0),
    ceiling("pan", "shapes", 0),
    ceiling("idle", "frames", 0),
];
/// A box, its handles and a few lines cost a handful of quads and paths. With all of them on
/// screen, the dashes of a marquee are most of it.
const PICTURE_OVERLAYS: &[Ceiling] = &[
    ceiling("*", "instances", 300),
    ceiling("band", "builds", 0),
    ceiling("band", "layouts", 0),
    ceiling("band", "shapes", 0),
    ceiling("handles", "builds", 0),
    ceiling("handles", "layouts", 0),
    ceiling("handles", "shapes", 0),
    ceiling("crop", "builds", 0),
    ceiling("crop", "layouts", 0),
    ceiling("crop", "shapes", 0),
    ceiling("divider", "builds", 0),
    ceiling("divider", "layouts", 0),
    ceiling("divider", "shapes", 0),
    ceiling("protractor", "builds", 1),
    ceiling("idle", "frames", 0),
];
/// Three channels of 256 bins, twice, a curve and the rest: one rebuild per step of a drag,
/// no text shaped anew and the disc never sent again.
const IMAGE_TOOLS: &[Ceiling] = &[
    ceiling("*", "instances", 2_800),
    ceiling("curve", "builds", 1),
    ceiling("curve", "shapes", 0),
    ceiling("curve", "uploaded_bytes", 400_000),
    ceiling("wheel", "builds", 1),
    ceiling("wheel", "shapes", 0),
    ceiling("idle", "frames", 0),
];

/// Whatever the panels show, a bar moved makes no title be shaped again.
const DOCKING: &[Ceiling] = &[ceiling("divider", "shapes", 0), ceiling("idle", "frames", 0)];

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

/// What a point of sale keeps on screen all day: a field for a scanner and a bar of keys.
fn counter_keys(interaction: &str) -> Setup {
    let key = |number| Hotkey::key(ravenrs_core::Key::Function(number));
    let bar = KeyBar::new()
        .item(KeyBarItem::new(key(1), "Atalhos", || {}))
        .item(KeyBarItem::new(key(6), "Cancelar item", || {}).disabled(true))
        .item(KeyBarItem::new(key(8), "Nota grande", || {}))
        .item(KeyBarItem::new(key(10), "Pendentes", || {}))
        .item(KeyBarItem::new(key(11), "Cancelar venda", || {}))
        .item(KeyBarItem::new(key(9), "Finalizar", || {}).primary());
    let field = ScanField::new(|_scan| {}).placeholder("Passe o produto");
    Setup {
        root: Box::new(VStack::new((field, bar)).spacing(16.0)),
        step: common(interaction, Point::new(300.0, 80.0), Point::new(900.0, 80.0)),
    }
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
        scene("virtual-list", &["VirtualList"], Class::Virtualized, lists, virtual_list, VIRTUALIZED),
        scene("filmstrip", &["Filmstrip"], Class::Virtualized, lists, filmstrip, VIRTUALIZED),
        scene("chart", &["Chart"], Class::Picture, CHART, chart, STILL),
        scene("chart-bars", &["Chart", "BarChart"], Class::Picture, CHART, chart_bars, STILL),
        scene("chart-board", &["Chart", "AreaChart"], Class::Picture, CHART, chart_board, STILL),
        scene("chart-sankey", &["SankeyChart"], Class::Static, CHART, chart_sankey, STILL),
        scene("lottie", &["Lottie"], Class::Animated, &["animate", "hover", "scroll"], lottie, ANIMATED),
        scene(
            "zoom-pan",
            &["ZoomPanView"],
            Class::Static,
            &["pan", "hover", "idle", "resize"],
            zoom_pan,
            ZOOM_PAN,
        ),
        scene(
            "image-tools",
            &["Histogram", "CurveEditor", "ColorWheel", "ZoomControl", "Minimap", "Ruler"],
            Class::Static,
            &["curve", "wheel", "idle", "resize"],
            image_tools,
            IMAGE_TOOLS,
        ),
        scene(
            "picture-overlays",
            &[
                "RubberBand",
                "TransformHandles",
                "CropOverlay",
                "Protractor",
                "CompareView",
                "RuleOfThirdsGrid",
            ],
            Class::Static,
            &["band", "handles", "crop", "divider", "protractor", "hover", "idle", "resize"],
            picture_overlays,
            PICTURE_OVERLAYS,
        ),
        scene("marquee", &["Marquee"], Class::Animated, &["animate"], marquee, ANIMATED),
        scene(
            "docking",
            &["DockArea"],
            Class::Static,
            &["hover", "tab", "divider", "idle", "resize"],
            docking,
            DOCKING,
        ),
        scene("images", &["Image", "Grid"], Class::Static, &["hover", "scroll", "idle"], images, STILL),
        scene(
            "counter-keys",
            &["KeyBar", "ScanField"],
            Class::Static,
            &["hover", "idle", "resize"],
            counter_keys,
            STILL,
        ),
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
