//! Arranging views on the page.

use ravenrs::layout::Axis;
use ravenrs::prelude::*;

use super::picture;

samples! {
    "VStack" => v_stack, "HStack" => h_stack, "Grid" => grid, "Spacer" => spacer,
    "ScrollView" => scroll_view, "ScrollScene" => scroll_scene, "ScrollLinked" => scroll_linked,
    "Sticky" => sticky, "Ticker" => ticker, "CountUp" => count_up, "Anchor" => anchor,
    "QuadField" => quad_field, "Splitter" => splitter, "SplitPanes" => split_panes,
    "SplitHandle" => split_handle, "DockArea" => dock_area, "Card" => card, "GlassCard" => glass_card,
    "Section" => section, "Divider" => divider, "AspectRatio" => aspect_ratio, "Collapse" => collapse,
    "ScrollEdgeArrow" => scroll_edge_arrow, "ZStack" => z_stack,
}

// sample: VStack, 150
fn v_stack() -> impl View {
    VStack::new((Text::new("First"), Text::new("Second"), Button::new("Continue"))).spacing(12.0)
}
// end

// sample: HStack, 90
fn h_stack() -> impl View {
    HStack::new((Text::new("First"), Text::new("Second"), Button::new("Continue")))
        .spacing(12.0)
        .align(Align::Center)
}
// end

// sample: Grid, 170
fn grid() -> impl View {
    let cell = |n: u32| Card::new(Text::new(n.to_string())).padding(16.0);
    Grid::new(3, (cell(1), cell(2), cell(3), cell(4), cell(5), cell(6))).spacing(8.0)
}
// end

// sample: Spacer, 90
fn spacer() -> impl View {
    HStack::new((Text::new("Left"), Spacer::new(), Button::new("Next"))).align(Align::Center)
}
// end

// sample: ScrollView, 220
fn scroll_view() -> impl View {
    let rows: Vec<AnyView> = (1..=30).map(|n| Box::new(Text::new(format!("Row {n}"))) as AnyView).collect();
    ScrollView::new(rows).spacing(8.0).height(180.0)
}
// end

// sample: ScrollScene, 320
fn scroll_scene() -> impl View {
    // Each step fades in, holds for its share of the scene and fades out.
    let step = |title: &'static str, text: &'static str, from: f32, to: f32| {
        let away = Effect::hidden().then(Effect::moved(0.0, 16.0));
        let card = Card::new(
            VStack::new((
                Text::new(title).style(TextStyle::Title),
                Text::new(text).foreground(Semantic::InkMuted),
            ))
            .spacing(8.0),
        );
        let link = ScrollLink::keyframes([
            (from - 0.08, away),
            (from + 0.02, Effect::IDENTITY),
            (to - 0.02, Effect::IDENTITY),
            (to + 0.08, away),
        ]);
        ScrollLinked::new(card, link)
    };
    let steps = ZStack::new((
        step("1. State", "A click changes a value.", -1.0, 0.33),
        step("2. Layout", "Only what read it moves.", 0.33, 0.66),
        step("3. Paint", "The GPU draws the frame.", 0.66, 2.0),
    ));
    ScrollView::new((
        Text::new("Scroll down: the scene holds while its steps change.").foreground(Semantic::InkMuted),
        ScrollScene::new(steps).length(600.0),
        Text::new("Here the scene lets go.").foreground(Semantic::InkMuted),
        Canvas::new(|_, _| {}).size(10.0, 200.0),
    ))
    .spacing(12.0)
    .height(280.0)
}
// end

// sample: ScrollLinked, 260
fn scroll_linked() -> impl View {
    let card = |text: &'static str| {
        ScrollLinked::new(Card::new(Text::new(text)), ScrollLink::fade_in().range(ScrollRange::Enter))
    };
    ScrollView::new((
        card("Each card fades in"),
        card("and rises into place"),
        card("as it enters the view,"),
        card("following the scroll"),
        card("and not a clock."),
    ))
    .spacing(80.0)
    .height(220.0)
}
// end

// sample: Sticky, 260
fn sticky() -> impl View {
    let section = |name: &'static str| {
        let rows: Vec<AnyView> =
            (1..=6).map(|n| Box::new(Text::new(format!("{name} {n}"))) as AnyView).collect();
        VStack::new((Sticky::new(Badge::new(name).tone(Tone::Accent)), VStack::new(rows).spacing(8.0)))
            .spacing(8.0)
    };
    ScrollView::new((section("Inputs"), section("Overlays"), section("Data"))).spacing(16.0).height(220.0)
}
// end

// sample: Ticker, 120
fn ticker() -> impl View {
    let chips = HStack::new((
        Badge::new("Button").tone(Tone::Accent),
        Badge::new("DataGrid"),
        Badge::new("Scheduler").tone(Tone::Success),
        Badge::new("Ribbon"),
        Badge::new("Chart").tone(Tone::Warning),
        Badge::new("CommandPalette"),
    ))
    .spacing(12.0);
    VStack::new((Ticker::new(chips).speed(40.0).gap(12.0),)).padding(8.0)
}
// end

// sample: CountUp, 140
fn count_up() -> impl View {
    HStack::new((
        VStack::new((
            Text::new("ROWS").style(TextStyle::Caption).foreground(Semantic::InkMuted),
            CountUp::new(100_000.0),
        ))
        .spacing(4.0),
        VStack::new((
            Text::new("COMPONENTS").style(TextStyle::Caption).foreground(Semantic::InkMuted),
            CountUp::new(160.0).format(|v| format!("{v:.0}+")),
        ))
        .spacing(4.0),
    ))
    .spacing(48.0)
}
// end

// sample: Anchor, 240
fn anchor() -> impl View {
    let rows: Vec<AnyView> = (1..=12).map(|n| Box::new(Text::new(format!("Row {n}"))) as AnyView).collect();
    ScrollView::new((
        Link::new("Jump to the summary", "#summary"),
        VStack::new(rows).spacing(8.0),
        Anchor::new("summary", Text::new("Summary").style(TextStyle::Title)),
        Canvas::new(|_, _| {}).size(10.0, 160.0),
    ))
    .spacing(12.0)
    .height(200.0)
}
// end

// sample: QuadField, 320
fn quad_field() -> impl View {
    // Scroll inside: the waving floor stands up into a small dashboard.
    // ravenrs-lint: allow(raw_color) the pixels of the scene, like those of an `ImageData::from_fn`
    let (floor, bars, ground) = (Color::hex(0x222734), Color::hex(0x4c66ee), Color::hex(0x12151c));
    let picture = move |u: f32, v: f32, time: f64| {
        let bar = ((u - 0.1) / 0.1).floor();
        let height = 0.35 + 0.25 * (bar * 1.7 + time as f32).sin().abs();
        if v < 0.15 {
            floor
        } else if (0.1..0.9).contains(&u) && v > 0.9 - height {
            bars
        } else {
            ground
        }
    };
    ScrollView::new((
        ScrollScene::new(QuadField::new(64, 36, picture).height(280.0)).length(600.0),
        Canvas::new(|_, _| {}).size(10.0, 400.0),
    ))
    .spacing(0.0)
    .height(280.0)
}
// end

// sample: Splitter, 230
fn splitter() -> impl View {
    Splitter::horizontal(Text::new("Explorer").padding(16.0), Text::new("Editor").padding(16.0))
        .ratio(0.32)
        .height(190.0)
}
// end

// sample: SplitPanes, 230
fn split_panes() -> impl View {
    SplitPanes::horizontal()
        .pane(SplitPane::new(Text::new("Sessions").padding(16.0)).size(180.0).range(120.0, 260.0))
        .pane(SplitPane::new(Text::new("Receipt").padding(16.0)).range(120.0, f32::MAX))
        .pane(SplitPane::new(Text::new("Activity").padding(16.0)).size(180.0).range(120.0, 260.0))
        .height(190.0)
}
// end

// sample: SplitHandle, 120
fn split_handle() -> impl View {
    let height = State::new(120.0_f32);
    // The size is yours to use; the bar only changes it.
    VStack::new((
        Text::new("Drag the bar, or focus it and use the arrow keys."),
        SplitHandle::new(Axis::Vertical, height.binding()).range(80.0, 300.0).inverted(),
    ))
    .spacing(8.0)
    .align(Align::Stretch)
}
// end

// sample: Card, 150
fn card() -> impl View {
    Card::new(
        VStack::new((
            Text::new("Quarterly report").style(TextStyle::Headline),
            Text::new("A grouped surface, lifted off the page.").foreground(Semantic::InkMuted),
        ))
        .spacing(4.0),
    )
    // Set it on the chosen card of a group.
    .selected(false)
}
// end

// sample: GlassCard, 150
fn glass_card() -> impl View {
    GlassCard::new(Text::new("Translucent, for Raven Glass compositions."))
}
// end

// sample: Section, 130
fn section() -> impl View {
    Section::new(
        "Notifications",
        (Text::new("Grouped content under a title.").foreground(Semantic::InkMuted),),
    )
    .action(Button::new("Edit").style(ButtonStyle::Secondary).size(ButtonSize::Small))
}
// end

// sample: Divider, 110
fn divider() -> impl View {
    VStack::new((Text::new("Above"), Divider::horizontal(), Text::new("Below")))
        .spacing(12.0)
        .align(Align::Stretch)
}
// end

// sample: AspectRatio, 190
fn aspect_ratio() -> impl View {
    VStack::new((AspectRatio::new(16.0 / 9.0, Image::new(picture(1.0)).stretch()),))
        .width(320.0)
        .align(Align::Stretch)
}
// end

// sample: Collapse, 170
fn collapse() -> impl View {
    let open = State::new(true);
    let flip = open.clone();
    view_fn(move || {
        let toggle = flip.clone();
        VStack::new((
            Button::new("Toggle").style(ButtonStyle::Secondary).on_click(move || toggle.update(|o| *o = !*o)),
            Collapse::new(open.get(), Text::new("The height animates, whatever the content.")),
        ))
        .spacing(12.0)
    })
}
// end

// sample: ScrollEdgeArrow, 110
fn scroll_edge_arrow() -> impl View {
    let photos: Vec<AnyView> = (1..=24)
        .map(|n| -> AnyView {
            Box::new(
                Text::new(format!("Photo {n}"))
                    .style(TextStyle::Callout)
                    .centered()
                    .frame(Some(120.0), Some(72.0))
                    .background(Semantic::Control)
                    .corner_radius(6.0),
            )
        })
        .collect();
    ScrollEdgeArrow::new(photos).fade(Semantic::SurfaceElevated)
}
// end

// sample: DockArea, 420
fn dock_area() -> impl View {
    let layout = State::new(
        DockLayout::new()
            .center(DockNode::tabs(["photo", "grid"]))
            .left(DockNode::tabs(["folders", "search"]), 220.0)
            .right(DockNode::vertical([DockNode::tabs(["layers"]), DockNode::tabs(["history"])]), 200.0)
            .bottom(DockNode::tabs(["log"]), 110.0),
    );
    let panels = vec![
        DockPanel::new("photo", "Photo").icon(IconName::Camera).closable(false),
        DockPanel::new("grid", "Grid"),
        DockPanel::new("folders", "Folders").icon(IconName::Folder),
        DockPanel::new("search", "Search").icon(IconName::Search),
        DockPanel::new("layers", "Layers"),
        DockPanel::new("history", "History"),
        DockPanel::new("log", "Log"),
    ];
    DockArea::new(layout.binding(), panels, |id| -> AnyView {
        Box::new(VStack::new((Text::new(format!("What {id} shows")),)).padding(12.0))
    })
    // Keep this text, and give it to DockLayout::parse the next time.
    .on_end(|layout| println!("{}", layout.to_text()))
    .height(420.0)
}
// end

// sample: ZStack, 110
fn z_stack() -> impl View {
    ZStack::new((
        Spacer::new().frame(Some(220.0), Some(80.0)).background(Semantic::Accent).corner_radius(8.0),
        Badge::new("On top").tone(Tone::Success),
    ))
    .align(Align::Center)
}
// end
