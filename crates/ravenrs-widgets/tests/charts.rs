//! Charts and barcodes: the layout comes from `ravenrs-graphics`, so these check
//! what the component adds. It paints a picture and text, and keeps them.
#![cfg(feature = "charts")]
#![allow(clippy::expect_used, reason = "test helpers fail loudly")]

use ravenrs_core::{ImageData, ImageQuad, Point, Primitive, Rect, Size};
use ravenrs_layout::Align;
use ravenrs_reactive::State;
use ravenrs_style::{Semantic, Theme};
use ravenrs_widgets::*;

fn ui(view: impl View + 'static) -> Ui {
    let mut ui = Ui::new(view, Theme::raven_dark());
    ui.set_viewport(Size::new(800.0, 600.0), 1.0);
    ui.update();
    ui
}

fn images(ui: &Ui) -> Vec<ImageQuad> {
    ui.scene()
        .primitives()
        .iter()
        .filter_map(|p| if let Primitive::Image(i) = p { Some(i.clone()) } else { None })
        .collect()
}

fn glyph_runs(ui: &Ui) -> usize {
    ui.scene().primitives().iter().filter(|p| matches!(p, Primitive::Glyphs(_))).count()
}

fn line() -> Chart {
    Chart::new(ChartKind::Line).categories(["A", "B", "C"]).series("S", [1.0, 3.0, 2.0])
}

fn pixel(image: &ImageData, x: u32, y: u32) -> [u8; 4] {
    let at = ((y * image.width() + x) * 4) as usize;
    let p = &image.pixels()[at..at + 4];
    [p[0], p[1], p[2], p[3]]
}

#[test]
fn every_kind_paints_a_picture() {
    for kind in [
        ChartKind::Bar,
        ChartKind::Line,
        ChartKind::Area,
        ChartKind::Pie,
        ChartKind::Doughnut,
        ChartKind::Scatter,
        ChartKind::Sparkline,
        ChartKind::SparkArea,
    ] {
        let chart = Chart::new(kind)
            .categories(["A", "B", "C"])
            .positions([1.0, 2.0, 3.0])
            .series("S", [1.0, 3.0, 2.0]);
        let ui = ui(VStack::new((chart,)).align(Align::Stretch));
        let image = images(&ui).into_iter().next().unwrap_or_else(|| panic!("{kind:?}"));
        assert!(image.image.pixels().chunks(4).any(|p| p[3] > 0), "{kind:?}");
    }
}

#[test]
fn the_labels_are_text_and_not_part_of_the_picture() {
    let ui = ui(VStack::new((line().title("Sales"),)).align(Align::Stretch));
    assert!(glyph_runs(&ui) >= 4, "title, axis and categories: {}", glyph_runs(&ui));
    let spark = Chart::new(ChartKind::Sparkline).series("S", [1.0, 2.0]);
    assert_eq!(glyph_runs(&self::ui(VStack::new((spark,)).align(Align::Stretch))), 0);
}

#[test]
fn the_picture_follows_the_screen_density() {
    let mut ui = ui(VStack::new((line().height(100.0),)).align(Align::Stretch));
    let one = images(&ui)[0].clone();
    ui.set_viewport(Size::new(800.0, 600.0), 2.0);
    ui.update();
    let two = images(&ui)[0].clone();
    assert_eq!(two.image.height(), one.image.height() * 2);
    assert_eq!(two.rect, one.rect, "the same logical size");
}

#[test]
fn a_rebuild_that_changes_nothing_uploads_nothing() {
    let tick = State::new(0);
    let values = State::new(vec![1.0, 3.0, 2.0]);
    let (t, v) = (tick.clone(), values.clone());
    // The format is a new closure on every build, as it is in any app.
    let mut ui = ui(view_fn(move || {
        let _ = t.get();
        VStack::new((Chart::new(ChartKind::Line)
            .categories(["A", "B", "C"])
            .series("S", v.get())
            .format(|value| format!("{value:.0}")),))
        .align(Align::Stretch)
    }));
    let first = images(&ui)[0].image.id();

    tick.set(1);
    ui.update();
    assert_eq!(images(&ui)[0].image.id(), first, "same drawing, same texture");

    values.set(vec![9.0, 1.0, 4.0]);
    ui.update();
    assert_ne!(images(&ui)[0].image.id(), first, "new numbers, new picture");
}

#[test]
fn a_new_size_redraws_and_moving_does_not() {
    let mut ui = ui(VStack::new((line().height(100.0),)).align(Align::Stretch));
    let first = images(&ui)[0].image.clone();
    ui.set_viewport(Size::new(500.0, 600.0), 1.0);
    ui.update();
    let narrow = images(&ui)[0].image.clone();
    assert_ne!(narrow.id(), first.id());
    assert_eq!(narrow.width(), 500);

    let offset = State::new(0.0);
    let o = offset.clone();
    let mut moving = self::ui(view_fn(move || {
        VStack::new((Spacer::new().height(o.get()), line().height(100.0))).align(Align::Stretch).spacing(0.0)
    }));
    let before = images(&moving)[0].clone();
    offset.set(40.0);
    moving.update();
    let after = images(&moving)[0].clone();
    assert_eq!(after.image.id(), before.image.id());
    assert_eq!(after.rect.y(), before.rect.y() + 40.0);
}

#[test]
fn a_chart_can_be_read_aloud() {
    let mut ui = ui(VStack::new((line().alt("Sales by month"),)).align(Align::Stretch));
    let tree = ui.access_tree();
    let node = tree
        .nodes
        .iter()
        .find(|n| n.name.as_deref() == Some("Sales by month"))
        .expect("the chart in the tree");
    assert_eq!(node.description.as_deref(), Some("A: 1, B: 3, C: 2"));
}

#[test]
fn nothing_to_draw_paints_nothing_and_does_not_fail() {
    let ui = ui(VStack::new((Chart::new(ChartKind::Pie), BarChart::new([] as [(&str, f32); 0])))
        .align(Align::Stretch));
    assert_eq!(glyph_runs(&ui), 0);
}

#[test]
fn bar_and_area_charts_keep_their_box() {
    let ui = ui(VStack::new((
        BarChart::new([("A", 1.0), ("B", 2.0)]).height(100.0),
        AreaChart::new([1.0, 2.0, 3.0]).tone(Semantic::Danger),
    ))
    .align(Align::Stretch));
    let found = images(&ui);
    assert_eq!(found.len(), 2);
    assert!(found[0].rect.height() > 100.0, "the labels sit under the bars");
    assert_eq!(found[1].rect.height(), 120.0);
    let danger = Theme::raven_dark().color(Semantic::Danger);
    let to8 = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    let red = to8(danger.r);
    assert!(found[1].image.pixels().chunks(4).any(|p| p[3] == 255 && p[0].abs_diff(red) < 4), "in its tone");
}

#[test]
fn a_qr_code_lands_on_whole_pixels() {
    let ui = ui(HStack::new((Barcode::qr("RavenRS").size(167.0, 167.0),)));
    let image = images(&ui)[0].image.clone();
    assert_eq!(image.width(), 167);
    // Paper is opaque white, modules opaque black: nothing in between but the rounded paper corners.
    let (mut dark, mut light) = (0, 0);
    for y in 6..161 {
        for x in 6..161 {
            match pixel(&image, x, y) {
                [0, 0, 0, 255] => dark += 1,
                [255, 255, 255, 255] => light += 1,
                other => panic!("({x}, {y}) is {other:?}"),
            }
        }
    }
    assert!(dark > 1000 && light > 1000, "{dark} {light}");
}

#[test]
fn a_linear_code_prints_its_text_unless_told_not_to() {
    let with = ui(HStack::new((Barcode::new(Symbology::Ean13, "789123456789"),)));
    assert_eq!(glyph_runs(&with), 1);
    let without = ui(HStack::new((Barcode::new(Symbology::Ean13, "789123456789").hide_text(),)));
    assert_eq!(glyph_runs(&without), 0);
    assert_eq!(images(&without)[0].rect.size, Size::new(240.0, 110.0));
}

#[test]
fn text_the_format_cannot_hold_says_so() {
    let ui = ui(HStack::new((Barcode::new(Symbology::Ean13, "letters"),)));
    assert!(images(&ui).is_empty());
    assert_eq!(glyph_runs(&ui), 1);
}

#[test]
fn a_code_stays_where_its_box_is() {
    let ui = ui(VStack::new((Spacer::new().height(30.0), Barcode::qr("x"))).spacing(0.0));
    assert_eq!(images(&ui)[0].rect.origin, Point::new(0.0, 30.0));
    assert_eq!(images(&ui)[0].rect, Rect::new(0.0, 30.0, 160.0, 160.0));
}

fn bars() -> Chart {
    Chart::new(ChartKind::Bar)
        .categories(["A", "B", "C", "D"])
        .series("First", [1.0, 2.0, 3.0, 4.0])
        .series("Second", [5.0, 6.0, 7.0, 8.0])
        .alt("Sales")
}

fn chart_ui(chart: Chart) -> Ui {
    ui(VStack::new((chart,)).align(Align::Stretch))
}

fn image_node(ui: &mut Ui) -> ravenrs_core::AccessNode {
    let tree = ui.access_tree();
    let found = tree.nodes.iter().find(|n| n.role == ravenrs_core::Role::Image);
    found.expect("the chart").clone()
}

fn hover(ui: &mut Ui, x: f32, y: f32) {
    ui.handle_event(&ravenrs_core::Event::PointerMove { position: Point::new(x, y) });
    ui.update();
    ui.update();
}

fn key(ui: &mut Ui, key: ravenrs_core::Key) {
    ui.handle_event(&ravenrs_core::Event::KeyDown { key, modifiers: ravenrs_core::Modifiers::default() });
    ui.update();
    ui.update();
}

#[test]
fn the_pointer_says_what_is_under_it() {
    let mut ui = chart_ui(bars());
    let before = glyph_runs(&ui);
    hover(&mut ui, 700.0, 100.0);
    assert_eq!(image_node(&mut ui).value.as_deref(), Some("D, First: 4, Second: 8"));
    assert!(glyph_runs(&ui) > before, "the tooltip is on the screen");
    hover(&mut ui, 100.0, 100.0);
    assert_eq!(image_node(&mut ui).value.as_deref(), Some("A, First: 1, Second: 5"));
    hover(&mut ui, 400.0, 500.0);
    assert_eq!(image_node(&mut ui).value, None, "the pointer left");
}

#[test]
fn a_tooltip_shows_what_it_was_asked_to() {
    let tooltip = ChartTooltip::new()
        .label_format(|name| format!("Week {name}"))
        .value_format(|value| format!("{value:.0} kcal"))
        .show_total("Total");
    let mut ui = chart_ui(bars().tooltip(tooltip));
    hover(&mut ui, 100.0, 100.0);
    assert_eq!(
        image_node(&mut ui).value.as_deref(),
        Some("Week A, First: 1 kcal, Second: 5 kcal, Total: 6 kcal")
    );
}

#[test]
fn the_arrow_keys_walk_the_categories() {
    let mut ui = chart_ui(bars());
    key(&mut ui, ravenrs_core::Key::Tab);
    key(&mut ui, ravenrs_core::Key::ArrowRight);
    assert_eq!(image_node(&mut ui).value.as_deref(), Some("A, First: 1, Second: 5"));
    key(&mut ui, ravenrs_core::Key::ArrowRight);
    assert_eq!(image_node(&mut ui).value.as_deref(), Some("B, First: 2, Second: 6"));
    key(&mut ui, ravenrs_core::Key::End);
    assert_eq!(image_node(&mut ui).value.as_deref(), Some("D, First: 4, Second: 8"));
    key(&mut ui, ravenrs_core::Key::Escape);
    assert_eq!(image_node(&mut ui).value, None);
}

#[test]
fn a_screen_reader_hears_every_series() {
    let mut ui = chart_ui(bars());
    let description = image_node(&mut ui).description.expect("the values");
    assert!(description.contains("First: A: 1") && description.contains("Second: A: 5"), "{description}");
}

#[test]
fn the_pointer_does_not_draw_the_picture_again() {
    let mut ui = chart_ui(bars());
    let first = images(&ui)[0].image.clone();
    hover(&mut ui, 100.0, 100.0);
    hover(&mut ui, 700.0, 100.0);
    assert_eq!(images(&ui)[0].image, first);
}

#[test]
fn a_still_chart_asks_for_no_frames() {
    let mut ui = chart_ui(bars());
    ui.update();
    assert!(!ui.needs_redraw());
    hover(&mut ui, 100.0, 100.0);
    let mut time = 0.0;
    while ui.is_animating() && time < 3.0 {
        time += 0.1;
        ui.set_time(time);
        ui.update();
    }
    assert!(!ui.needs_redraw());
}

#[test]
fn without_a_tooltip_it_is_only_a_picture() {
    let mut ui = chart_ui(bars().hide_tooltip());
    key(&mut ui, ravenrs_core::Key::Tab);
    key(&mut ui, ravenrs_core::Key::ArrowRight);
    hover(&mut ui, 100.0, 100.0);
    assert_eq!(image_node(&mut ui).value, None);
}

#[test]
fn the_new_kinds_paint_a_picture_and_say_what_is_under_the_pointer() {
    for kind in [ChartKind::Radar, ChartKind::RadialBar] {
        let chart = Chart::new(kind).categories(["A", "B", "C"]).series("S", [1.0, 3.0, 2.0]);
        let mut ui = chart_ui(chart);
        assert!(images(&ui)[0].image.pixels().chunks(4).any(|p| p[3] > 0), "{kind:?}");
        hover(&mut ui, 460.0, 80.0);
        assert!(image_node(&mut ui).value.is_some(), "{kind:?}");
    }
}

#[test]
fn icons_stand_in_for_dots() {
    let plain = chart_ui(line());
    let with_icons = chart_ui(line().dot_icon(IconName::Star));
    assert_eq!(glyph_runs(&with_icons), glyph_runs(&plain) + 3);
}

#[test]
fn the_series_colors_are_the_themes_and_not_status_colors() {
    let theme = Theme::raven_light();
    let colors = [Semantic::Chart1, Semantic::Chart2, Semantic::Chart3, Semantic::Chart4, Semantic::Chart5]
        .map(|role| theme.color(role));
    assert_eq!(colors[0], theme.color(Semantic::Accent));
    for color in &colors[1..] {
        assert_ne!(*color, theme.color(Semantic::Danger));
        assert_ne!(*color, theme.color(Semantic::Success));
    }
    assert_ne!(colors[1], Theme::raven_dark().color(Semantic::Chart2), "each theme has its own");
}

#[test]
fn a_chart_card_says_what_it_is_and_switches_its_tabs() {
    let tab = State::new(0_usize);
    let card = ChartCard::new(line())
        .title("Sales")
        .description("Half a year")
        .trend("Up by 5%", IconName::TrendingUp)
        .note("All channels")
        .tabs(tab.binding(), [("Desktop", "120"), ("Mobile", "80")]);
    let mut ui = ui(VStack::new((card,)).align(Align::Stretch));
    let tree = ui.access_tree();
    let names: Vec<&str> = tree.nodes.iter().filter_map(|n| n.name.as_deref()).collect();
    for text in ["Sales", "Half a year", "Up by 5%", "All channels", "Mobile: 80"] {
        assert!(names.contains(&text), "{text} in {names:?}");
    }
    let id = tree.nodes.iter().find(|n| n.name.as_deref() == Some("Mobile: 80")).expect("the tab").id;
    ui.perform_action(id, ravenrs_core::AccessAction::Click);
    ui.update();
    assert_eq!(tab.get(), 1);
}

/// Where the tooltip's panel is: the one thing in a chart with a shadow.
fn panel_of(ui: &Ui) -> Option<Rect> {
    ui.scene().primitives().iter().find_map(|p| match p {
        Primitive::Quad(q) if q.shadow.is_some() => Some(q.rect),
        _ => None,
    })
}

fn at_time(ui: &mut Ui, time: f64) {
    ui.set_time(time);
    ui.update();
}

#[test]
fn the_tooltip_glides_from_one_place_to_the_next() {
    let mut ui = chart_ui(bars());
    at_time(&mut ui, 0.0);
    ui.handle_event(&ravenrs_core::Event::PointerMove { position: Point::new(100.0, 100.0) });
    ui.update();
    ui.update();
    at_time(&mut ui, 3.0);
    let start = panel_of(&ui).expect("the panel").x();
    ui.handle_event(&ravenrs_core::Event::PointerMove { position: Point::new(300.0, 100.0) });
    ui.update();
    ui.update();
    at_time(&mut ui, 3.05);
    let moving = panel_of(&ui).expect("the panel").x();
    for frame in 2..=120 {
        at_time(&mut ui, 3.0 + f64::from(frame) / 30.0);
    }
    let end = panel_of(&ui).expect("the panel").x();
    assert!(end > start + 50.0, "{start} to {end}");
    assert!(moving > start && moving < end, "{start}, {moving}, {end}: on its way, not there yet");
}

#[test]
fn the_tooltip_follows_the_pointer_up_and_down_a_column() {
    let mut ui = chart_ui(bars());
    at_time(&mut ui, 0.0);
    ui.handle_event(&ravenrs_core::Event::PointerMove { position: Point::new(100.0, 60.0) });
    ui.update();
    ui.update();
    at_time(&mut ui, 3.0);
    let high = panel_of(&ui).expect("the panel").center().y;
    ui.handle_event(&ravenrs_core::Event::PointerMove { position: Point::new(100.0, 180.0) });
    ui.update();
    ui.update();
    for frame in 1..=120 {
        at_time(&mut ui, 3.0 + f64::from(frame) / 30.0);
    }
    let low = panel_of(&ui).expect("the panel").center().y;
    // Past the middle of the chart it also goes from below the pointer to above it.
    assert!(low > high + 30.0, "{high} to {low}");
}

fn settled_panel(ui: &mut Ui, x: f32, y: f32) -> Rect {
    at_time(ui, 0.0);
    move_to(ui, x, y);
    settle(ui, 0.0);
    panel_of(ui).expect("the panel")
}

#[test]
fn the_tooltip_keeps_to_the_side_with_the_most_room() {
    for (x, y) in [(100.0, 40.0), (700.0, 40.0), (100.0, 170.0), (700.0, 170.0)] {
        let panel = settled_panel(&mut chart_ui(bars()), x, y);
        let point = Point::new(x, y);
        assert!(!panel.contains(point), "{panel:?} covers {point:?}");
        // Each of the four categories owns a quarter of the width, and it stays out of it.
        if x < 400.0 {
            assert!(panel.x() >= 200.0, "to the right: {panel:?}");
        } else {
            assert!(panel.max_x() <= 600.0, "to the left: {panel:?}");
        }
        if y < 100.0 {
            assert!(panel.center().y > y, "below: {panel:?}");
        } else {
            assert!(panel.center().y < y, "above: {panel:?}");
        }
    }
}

#[test]
fn a_tooltip_can_say_the_share_of_a_slice() {
    let pie = Chart::new(ChartKind::Pie)
        .categories(["A", "B"])
        .series("S", [1.0, 3.0])
        .tooltip(ChartTooltip::new().show_percent());
    let mut ui = chart_ui(pie);
    key(&mut ui, ravenrs_core::Key::Tab);
    key(&mut ui, ravenrs_core::Key::End);
    assert_eq!(image_node(&mut ui).value.as_deref(), Some("B: 3 (75%)"));
}

#[test]
fn a_tooltip_can_say_the_share_in_a_category() {
    let mut ui = chart_ui(bars().tooltip(ChartTooltip::new().show_percent()));
    hover(&mut ui, 100.0, 100.0);
    assert_eq!(image_node(&mut ui).value.as_deref(), Some("A, First: 1 (17%), Second: 5 (83%)"));
}

#[test]
fn a_value_can_have_a_color_and_a_row_can_go_without_its_square() {
    let danger = Theme::raven_dark().color(Semantic::Danger);
    let tones = |ui: &Ui| {
        ui.scene()
            .primitives()
            .iter()
            .filter(|p| matches!(p, Primitive::Glyphs(g) if g.color == danger))
            .count()
    };
    let squares = |ui: &Ui| {
        ui.scene()
            .primitives()
            .iter()
            .filter(|p| matches!(p, Primitive::Quad(q) if q.rect.width() == 10.0 && q.rect.height() == 10.0))
            .count()
    };
    let mut plain = chart_ui(bars());
    hover(&mut plain, 100.0, 100.0);
    assert_eq!(tones(&plain), 0);
    assert_eq!(squares(&plain), 2);
    let tooltip = ChartTooltip::new()
        .value_color(|series, _| (series == 1).then_some(Semantic::Danger))
        .no_indicator_for("Second");
    let mut ui = chart_ui(bars().tooltip(tooltip));
    hover(&mut ui, 100.0, 100.0);
    assert_eq!(tones(&ui), 1);
    assert_eq!(squares(&ui), 1);
}

#[test]
fn a_tooltip_can_be_written_by_hand_and_is_still_read_aloud() {
    let tooltip = ChartTooltip::new()
        .content(|index, name| -> AnyView { Box::new(Text::new(format!("Hand made {index} {name}"))) });
    let mut ui = chart_ui(bars().tooltip(tooltip));
    hover(&mut ui, 100.0, 100.0);
    let tree = ui.access_tree();
    let names: Vec<&str> = tree.nodes.iter().filter_map(|n| n.name.as_deref()).collect();
    assert!(names.contains(&"Sales"), "{names:?}");
    assert_eq!(image_node(&mut ui).value.as_deref(), Some("A, First: 1, Second: 5"));
    let plain = chart_ui(bars());
    assert!(glyph_runs(&ui) > glyph_runs(&plain), "the text of the panel is on the screen");
    assert!(panel_of(&ui).is_some(), "in the panel of a tooltip");
}

#[test]
fn a_chart_that_comes_back_keeps_its_picture() {
    let picture = |ui: &Ui| {
        ui.scene().primitives().iter().find_map(|p| match p {
            ravenrs_core::Primitive::Image(image) => Some(image.image.id()),
            _ => None,
        })
    };
    let open = ravenrs_reactive::State::new(true);
    let showing = open.clone();
    let mut ui = Ui::new(
        view_fn(move || -> AnyView {
            if showing.get() {
                Box::new(AreaChart::new([1.5, 2.5, 3.5, 9.25]))
            } else {
                Box::new(Text::new("Another screen"))
            }
        }),
        ravenrs_style::Theme::raven_light(),
    );
    ui.set_viewport(ravenrs_core::Size::new(640.0, 400.0), 2.0);
    ui.update();
    let first = picture(&ui).expect("a picture");
    open.set(false);
    ui.update();
    assert_eq!(picture(&ui), None);
    open.set(true);
    ui.update();
    assert_eq!(picture(&ui), Some(first), "the same image, so nothing is drawn or uploaded again");
}

fn settle(ui: &mut Ui, from: f64) -> f64 {
    let mut time = from;
    for _ in 0..120 {
        time += 1.0 / 30.0;
        at_time(ui, time);
    }
    time
}

fn move_to(ui: &mut Ui, x: f32, y: f32) {
    ui.handle_event(&ravenrs_core::Event::PointerMove { position: Point::new(x, y) });
    ui.update();
    ui.update();
}

fn opacity(ui: &Ui) -> f32 {
    images(ui)[0].opacity
}

fn slices_on_top(ui: &Ui) -> usize {
    ui.scene().primitives().iter().filter(|p| matches!(p, Primitive::Path(_))).count()
}

/// The quads in the color of a bar, drawn over the picture.
fn bars_on_top(ui: &Ui, color: ravenrs_core::Color) -> Vec<Rect> {
    let primitives = ui.scene().primitives();
    let start = primitives.iter().position(|p| matches!(p, Primitive::Image(_))).unwrap_or(0);
    primitives[start..]
        .iter()
        .filter_map(|p| match p {
            // Not the tooltip's square nor the legend's.
            Primitive::Quad(q) if q.fill == color.into() && q.rect.height() > 12.0 => Some(q.rect),
            _ => None,
        })
        .collect()
}

#[test]
fn the_rest_of_the_bars_fade_around_the_one_under_the_pointer() {
    let mut ui = chart_ui(bars());
    at_time(&mut ui, 0.0);
    assert_eq!(opacity(&ui), 1.0, "still, it is as it was drawn");
    move_to(&mut ui, 100.0, 100.0);
    let time = settle(&mut ui, 0.0);
    assert!(opacity(&ui) < 0.7, "{}", opacity(&ui));
    let first = Theme::raven_dark().color(Semantic::Chart1);
    assert_eq!(bars_on_top(&ui, first).len(), 1, "only the bar in focus stays at full strength");

    move_to(&mut ui, 400.0, 500.0);
    settle(&mut ui, time);
    assert_eq!(opacity(&ui), 1.0, "and it all comes back");
    assert!(bars_on_top(&ui, first).is_empty());
    assert!(!ui.needs_redraw(), "and it stops asking for frames");
}

#[test]
fn focus_can_be_turned_off() {
    let mut ui = chart_ui(bars().no_hover_focus());
    at_time(&mut ui, 0.0);
    move_to(&mut ui, 100.0, 100.0);
    settle(&mut ui, 0.0);
    assert_eq!(opacity(&ui), 1.0);
    assert_eq!(image_node(&mut ui).value.as_deref(), Some("A, First: 1, Second: 5"), "the tooltip stays");

    let pie = Chart::new(ChartKind::Pie).categories(["A", "B"]).series("S", [1.0, 1.0]);
    let mut ui = chart_ui(pie.no_hover_focus());
    at_time(&mut ui, 0.0);
    move_to(&mut ui, 500.0, 100.0);
    settle(&mut ui, 0.0);
    assert_eq!(slices_on_top(&ui), 0);
}

#[test]
fn a_slice_comes_forward_without_making_the_pie_smaller() {
    let pie = || Chart::new(ChartKind::Pie).categories(["A", "B"]).series("S", [1.0, 1.0]).hide_legend();
    let plain = chart_ui(pie().no_hover_focus());
    let mut ui = chart_ui(pie());
    assert_eq!(images(&ui)[0].image.pixels(), images(&plain)[0].image.pixels(), "same pie when still");
    at_time(&mut ui, 0.0);
    move_to(&mut ui, 500.0, 100.0);
    assert_eq!(slices_on_top(&ui), 0, "it rises from where it is");
    settle(&mut ui, 0.0);
    assert_eq!(slices_on_top(&ui), 1);
    assert!(opacity(&ui) < 0.7);
    assert_eq!(images(&ui)[0].image.pixels(), images(&plain)[0].image.pixels(), "the picture is untouched");
}

/// Where the cursor of a line chart is: the thin upright line over the picture.
fn cursor_of(ui: &Ui) -> Option<ravenrs_core::Quad> {
    let primitives = ui.scene().primitives();
    let start = primitives.iter().position(|p| matches!(p, Primitive::Image(_)))?;
    primitives[start..].iter().find_map(|p| match p {
        Primitive::Quad(q) if q.rect.width() <= 1.0 && q.rect.height() > 50.0 => Some(*q),
        _ => None,
    })
}

#[test]
fn the_cursor_glides_between_categories_and_fades_when_the_pointer_leaves() {
    let mut ui = chart_ui(line().hide_legend());
    at_time(&mut ui, 0.0);
    move_to(&mut ui, 20.0, 100.0);
    let time = settle(&mut ui, 0.0);
    let start = cursor_of(&ui).expect("the cursor").rect.x();

    move_to(&mut ui, 780.0, 100.0);
    at_time(&mut ui, time + 0.03);
    let moving = cursor_of(&ui).expect("the cursor").rect.x();
    let time = settle(&mut ui, time + 0.03);
    let end = cursor_of(&ui).expect("the cursor").rect.x();
    assert!(start < moving && moving < end, "{start}, {moving}, {end}");

    move_to(&mut ui, 400.0, 500.0);
    assert!(cursor_of(&ui).is_some(), "it does not vanish at once");
    settle(&mut ui, time);
    assert!(cursor_of(&ui).is_none(), "it is gone in the end");
    assert!(!ui.needs_redraw());
}

#[test]
fn the_cursor_does_not_glide_in_from_where_it_last_was() {
    let mut ui = chart_ui(line().hide_legend());
    at_time(&mut ui, 0.0);
    move_to(&mut ui, 20.0, 100.0);
    let time = settle(&mut ui, 0.0);
    move_to(&mut ui, 400.0, 500.0);
    let time = settle(&mut ui, time);
    move_to(&mut ui, 780.0, 100.0);
    at_time(&mut ui, time + 0.03);
    let right_away = cursor_of(&ui).expect("the cursor").rect.x();
    settle(&mut ui, time + 0.03);
    assert_eq!(right_away, cursor_of(&ui).expect("the cursor").rect.x());
}

#[test]
fn without_motion_the_cursor_goes_straight_to_the_category() {
    let mut ui = chart_ui(line().hide_legend().no_hover_motion());
    at_time(&mut ui, 0.0);
    move_to(&mut ui, 20.0, 100.0);
    let time = settle(&mut ui, 0.0);
    move_to(&mut ui, 780.0, 100.0);
    at_time(&mut ui, time + 0.03);
    let right_away = cursor_of(&ui).expect("the cursor").rect.x();
    settle(&mut ui, time + 0.03);
    assert_eq!(right_away, cursor_of(&ui).expect("the cursor").rect.x());
}

#[test]
fn gridlines_land_on_whole_pixels_wherever_the_chart_is() {
    let mut ui = ui(VStack::new((Spacer::new().height(10.3), line().height(100.0)))
        .align(Align::Stretch)
        .spacing(0.0));
    ui.set_viewport(Size::new(800.0, 600.0), 2.0);
    ui.update();
    let rules: Vec<Rect> = ui
        .scene()
        .primitives()
        .iter()
        .filter_map(|p| match p {
            Primitive::Quad(q) if q.rect.height() <= 1.0 && q.rect.width() > 100.0 => Some(q.rect),
            _ => None,
        })
        .collect();
    assert!(rules.len() >= 2, "{rules:?}");
    for rule in rules {
        let y = rule.y() * 2.0;
        assert!((y - y.round()).abs() < 0.001, "{rule:?}");
    }
}

#[test]
fn less_motion_means_no_glide_and_no_fade() {
    let mut ui = chart_ui(line().hide_legend());
    ui.set_reduced_motion(true);
    at_time(&mut ui, 0.0);
    move_to(&mut ui, 20.0, 100.0);
    at_time(&mut ui, 0.01);
    let start = cursor_of(&ui).expect("right away").rect.x();
    move_to(&mut ui, 780.0, 100.0);
    at_time(&mut ui, 0.02);
    let end = cursor_of(&ui).expect("right away").rect.x();
    settle(&mut ui, 0.02);
    assert_eq!(end, cursor_of(&ui).expect("still there").rect.x());
    assert!(end > start + 100.0);
    move_to(&mut ui, 400.0, 500.0);
    at_time(&mut ui, 5.0);
    assert!(cursor_of(&ui).is_none(), "gone at once");
}

fn thin_lines(ui: &Ui) -> usize {
    ui.scene()
        .primitives()
        .iter()
        .filter(|p| matches!(p, Primitive::Quad(q) if q.rect.width() <= 1.0 || q.rect.height() <= 1.0))
        .count()
}

#[test]
fn the_grid_can_be_dashed_have_columns_and_more_steps() {
    let plain = thin_lines(&chart_ui(line()));
    assert!(plain > 0);
    assert!(thin_lines(&chart_ui(line().grid_dashed())) > plain * 10);
    assert_eq!(thin_lines(&chart_ui(line().grid_columns(4))), plain + 5);
    assert!(thin_lines(&chart_ui(line().value_ticks(10))) > plain);
}

#[test]
fn the_grid_has_a_token_of_its_own_that_follows_the_border() {
    let theme = Theme::raven_light();
    assert_eq!(theme.color(Semantic::ChartGrid), theme.color(Semantic::Border));
}

#[test]
fn the_room_for_the_axis_comes_from_the_real_width_of_its_numbers() {
    // Narrow digits and wide capitals take the same room in an estimate.
    let room = |format: fn(f64) -> String| {
        let ui = chart_ui(bars().show_value_axis().format(format));
        let primitives = ui.scene().primitives();
        primitives.iter().find_map(|p| match p {
            Primitive::Quad(q) if q.rect.height() <= 1.0 && q.rect.width() > 100.0 => Some(q.rect.x()),
            _ => None,
        })
    };
    let narrow = room(|_| "1111".to_owned()).expect("a gridline");
    let wide = room(|_| "WWWW".to_owned()).expect("a gridline");
    assert!(wide > narrow + 10.0, "{narrow} and {wide}");
}

#[test]
fn a_line_of_reference_is_drawn_over_the_data_and_says_what_it_is() {
    let plain = chart_ui(bars());
    let ui = chart_ui(bars().reference(ChartReference::new(5.0).label("Target").color(Semantic::Danger)));
    assert!(glyph_runs(&ui) > glyph_runs(&plain));
    let danger = Theme::raven_dark().color(Semantic::Danger);
    let primitives = ui.scene().primitives();
    let image = primitives.iter().position(|p| matches!(p, Primitive::Image(_))).expect("the picture");
    let dashes = primitives[image..]
        .iter()
        .filter(|p| matches!(p, Primitive::Quad(q) if q.fill == danger.into() && q.rect.height() <= 1.0))
        .count();
    assert!(dashes > 10, "{dashes}");
}

#[test]
fn a_fixed_domain_changes_the_picture_and_the_tooltip_keeps_the_real_value() {
    let mut ui = chart_ui(bars().value_domain(2.0, 5.0));
    let free = chart_ui(bars());
    assert_ne!(images(&ui)[0].image.pixels(), images(&free)[0].image.pixels());
    hover(&mut ui, 100.0, 100.0);
    assert_eq!(image_node(&mut ui).value.as_deref(), Some("A, First: 1, Second: 5"));
}

#[test]
fn a_line_past_a_fixed_domain_is_cut_off_at_the_edge() {
    let tall = Chart::new(ChartKind::Line)
        .categories(["A", "B", "C"])
        .series("S", [1.0, 100.0, 1.0])
        .hide_legend()
        .hide_category_axis();
    let cut = chart_ui(tall.clone().curve(ChartCurve::Linear).value_domain(0.0, 10.0));
    let image = images(&cut)[0].image.clone();
    // The line would leave through the top on its way to 100; nothing goes past it.
    let top_row = (0..image.width()).filter(|x| pixel(&image, *x, 0)[3] > 0).count();
    assert_eq!(top_row, 0);
    let any = image.pixels().chunks(4).any(|p| p[3] > 0);
    assert!(any, "what is inside stays");
}
