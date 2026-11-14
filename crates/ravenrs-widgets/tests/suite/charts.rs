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
fn every_kind_but_bars_paints_a_picture() {
    for kind in [
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
fn bars_are_quads_so_a_new_size_draws_no_picture() {
    let mut ui = chart_ui(bars());
    assert!(images(&ui).is_empty());
    let first = Theme::raven_dark().color(Semantic::Chart1);
    assert_eq!(bars_on_top(&ui, first).len(), 4);
    ui.set_viewport(Size::new(500.0, 600.0), 1.0);
    ui.update();
    assert!(images(&ui).is_empty());
    assert_eq!(bars_on_top(&ui, first).len(), 4);
}

#[test]
fn a_bar_that_fades_shows_no_gridline_through_it() {
    let mut ui = chart_ui(bars().bar_radius(0.0));
    at_time(&mut ui, 0.0);
    move_to(&mut ui, 700.0, 100.0);
    settle(&mut ui, 0.0);
    let primitives = ui.scene().primitives();
    let quads: Vec<&ravenrs_core::Quad> =
        primitives.iter().filter_map(|p| if let Primitive::Quad(q) = p { Some(q) } else { None }).collect();
    let chart1 = Theme::raven_dark().color(Semantic::Chart1);
    let faded: Vec<Rect> = quads
        .iter()
        .filter(|q| matches!(q.fill, ravenrs_core::Brush::Solid(c) if c.r == chart1.r && c.a < 1.0))
        .map(|q| q.rect)
        .collect();
    assert_eq!(faded.len(), 3, "the bars of the other categories");
    let lines = quads.iter().filter(|q| q.rect.height() <= 1.0 && q.rect.width() > 1.0).count();
    assert!(lines > 0, "the grid is there");
    for bar in faded {
        let inner = Rect::new(bar.x() + 0.5, bar.y() + 1.5, bar.width() - 1.0, bar.height() - 3.0);
        assert!(
            !quads.iter().any(|q| q.rect.height() <= 1.0 && !q.rect.intersect(&inner).is_empty()),
            "a line behind {bar:?}"
        );
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
    assert!(two.image.height().abs_diff(one.image.height() * 2) <= 2);
    assert!((two.rect.height() - one.rect.height()).abs() <= 1.0, "the same logical size");
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
    assert!(narrow.width() < first.width() && narrow.width() <= 500);

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
    assert_eq!(found.len(), 1, "bars are not a picture");
    assert!(found[0].rect.max_y() - found[0].rect.y() <= 120.0);
    assert!(
        ui.scene().primitives().iter().any(|p| matches!(p, Primitive::Quad(q) if q.rect.height() > 40.0))
    );
    let found = [found[0].clone(), found[0].clone()];
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

fn chart_ui_view(view: impl View + 'static) -> Ui {
    let mut ui = Ui::new(VStack::new((view,)).align(Align::Stretch), Theme::raven_dark());
    ui.set_viewport(Size::new(800.0, 600.0), 1.0);
    ui.update();
    ui
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
    let mut ui = chart_ui(line());
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
fn a_theme_that_chooses_a_series_draws_it_in_that_color() {
    let mut theme = Theme::raven_dark();
    let own = ravenrs_core::Color::hex(0xc2410c);
    theme.set_color(Semantic::Chart2, own);
    let mut ui = Ui::new(VStack::new((bars(),)).align(Align::Stretch), theme);
    ui.set_viewport(Size::new(800.0, 600.0), 1.0);
    ui.update();
    assert_eq!(bars_on_top(&ui, own).len(), 4, "the second series is the theme's own");
    assert!(bars_on_top(&ui, Theme::raven_dark().color(Semantic::Chart2)).is_empty());
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

/// How solid the bars of the theme's first color are, the faintest of them.
fn faintest_bar(ui: &Ui) -> f32 {
    let first = Theme::raven_dark().color(Semantic::Chart1);
    let alpha = ui.scene().primitives().iter().filter_map(|p| match p {
        Primitive::Quad(q) if q.rect.height() > 12.0 => match q.fill {
            ravenrs_core::Brush::Solid(c) if c.r == first.r && c.g == first.g => Some(c.a),
            _ => None,
        },
        _ => None,
    });
    alpha.fold(1.0, f32::min)
}

fn slices_on_top(ui: &Ui) -> usize {
    ui.scene().primitives().iter().filter(|p| matches!(p, Primitive::Path(_))).count()
}

/// The bars of this color at full strength.
fn bars_on_top(ui: &Ui, color: ravenrs_core::Color) -> Vec<Rect> {
    ui.scene()
        .primitives()
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
    assert_eq!(faintest_bar(&ui), 1.0, "still, it is as it was drawn");
    move_to(&mut ui, 100.0, 100.0);
    let time = settle(&mut ui, 0.0);
    assert!(faintest_bar(&ui) < 0.7, "{}", faintest_bar(&ui));
    let first = Theme::raven_dark().color(Semantic::Chart1);
    assert_eq!(bars_on_top(&ui, first).len(), 1, "only the bar in focus stays at full strength");

    move_to(&mut ui, 400.0, 500.0);
    settle(&mut ui, time);
    assert_eq!(faintest_bar(&ui), 1.0, "and it all comes back");
    assert_eq!(bars_on_top(&ui, first).len(), 4);
    assert!(!ui.needs_redraw(), "and it stops asking for frames");
}

#[test]
fn focus_can_be_turned_off() {
    let mut ui = chart_ui(bars().no_hover_focus());
    at_time(&mut ui, 0.0);
    move_to(&mut ui, 100.0, 100.0);
    settle(&mut ui, 0.0);
    assert_eq!(faintest_bar(&ui), 1.0);
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
    let bar = primitives.iter().rposition(|p| matches!(p, Primitive::Quad(q) if q.rect.height() > 40.0));
    let dashes = primitives[bar.expect("the bars")..]
        .iter()
        .filter(|p| matches!(p, Primitive::Quad(q) if q.fill == danger.into() && q.rect.height() <= 1.0))
        .count();
    assert!(dashes > 10, "{dashes}");
}

#[test]
fn a_fixed_domain_changes_the_bars_and_the_tooltip_keeps_the_real_value() {
    let mut ui = chart_ui(bars().value_domain(2.0, 5.0));
    let free = chart_ui(bars());
    let first = Theme::raven_dark().color(Semantic::Chart1);
    assert_ne!(bars_on_top(&ui, first), bars_on_top(&free, first));
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
    let shown = images(&cut)[0].clone();
    // The line would leave through the top on its way to 100; nothing goes past it.
    assert!(shown.rect.y() >= 3.0, "{:?}", shown.rect);
    let any = shown.image.pixels().chunks(4).any(|p| p[3] > 0);
    assert!(any, "what is inside stays");
}

#[test]
fn slices_have_a_real_gap_and_their_own_radius_and_labels_can_have_guides() {
    let pie = || {
        Chart::new(ChartKind::Pie)
            .categories(["A", "B", "C", "D"])
            .series("S", [5.0, 1.0, 1.0, 5.0])
            .hide_legend()
            .height(300.0)
    };
    let plain = chart_ui(pie());
    let reaching = chart_ui(pie().slice_radii([1.0, 0.5, 1.0, 1.0]));
    let filled = |ui: &Ui| images(ui)[0].image.pixels().chunks(4).filter(|p| p[3] > 128).count();
    assert!(filled(&reaching) < filled(&plain), "the second slice reaches less");

    let no_gap = chart_ui(pie().no_separator());
    assert!(filled(&no_gap) > filled(&plain), "the gap is empty, not painted");

    let labeled = pie().category_labels(ChartLabelAt::Outside);
    let bare = glyph_runs(&chart_ui(labeled.clone()));
    let guided = chart_ui(labeled.label_guides());
    assert_eq!(glyph_runs(&guided), bare, "the same labels");
    assert_ne!(images(&guided)[0].image.pixels(), images(&plain)[0].image.pixels());
}

#[test]
fn a_slice_short_of_the_edge_comes_forward_too() {
    let pie = Chart::new(ChartKind::Pie)
        .categories(["A", "B"])
        .series("S", [1.0, 1.0])
        .slice_radii([0.6, 1.0])
        .hide_legend();
    let mut ui = chart_ui(pie);
    at_time(&mut ui, 0.0);
    move_to(&mut ui, 460.0, 130.0);
    settle(&mut ui, 0.0);
    assert_eq!(slices_on_top(&ui), 1);
}

#[test]
fn bars_can_fill_from_tip_to_base_grow_from_any_side_and_keep_a_least_length() {
    let quad_of = |ui: &Ui| {
        ui.scene()
            .primitives()
            .iter()
            .find_map(|p| match p {
                Primitive::Quad(q) if !matches!(q.fill, ravenrs_core::Brush::Solid(_)) => Some(*q),
                _ => None,
            })
            .expect("a bar in a gradient")
    };
    let one = || Chart::new(ChartKind::Bar).categories(["A"]).series("S", [4.0]).hide_legend();
    let ui = chart_ui(one().gradient());
    assert!(quad_of(&ui).rect.height() > 40.0);

    let first = Theme::raven_dark().color(Semantic::Chart1);
    let two = || Chart::new(ChartKind::Bar).categories(["A", "B"]).series("S", [0.001, 100.0]).hide_legend();
    let all = |ui: &Ui| -> Vec<Rect> {
        ui.scene()
            .primitives()
            .iter()
            .filter_map(|p| match p {
                Primitive::Quad(q) if q.fill == first.into() => Some(q.rect),
                _ => None,
            })
            .collect()
    };
    assert!(all(&chart_ui(two().min_bar_length(9.0)))[0].height() >= 8.9);

    let top = chart_ui(two().bars_from(ChartBarFrom::Top));
    let found = all(&top);
    assert!((found[0].y() - found[1].y()).abs() < 0.01 && found[1].height() > found[0].height());

    let mut right = chart_ui(two().bars_from(ChartBarFrom::Right));
    let found = all(&right);
    assert!((found[0].max_x() - found[1].max_x()).abs() < 0.01 && found[1].width() > found[0].width());
    let at = found[1].center();
    hover(&mut right, at.x, at.y);
    assert_eq!(image_node(&mut right).value.as_deref(), Some("B, S: 100"));
}

#[test]
fn a_sparkline_stays_a_plain_picture_until_given_a_tooltip() {
    let spark = || Chart::new(ChartKind::Sparkline).categories(["A", "B", "C"]).series("S", [1.0, 3.0, 2.0]);
    let mut plain = chart_ui(spark().height(60.0));
    hover(&mut plain, 700.0, 30.0);
    assert_eq!(image_node(&mut plain).value, None);
    assert!(!plain.needs_redraw());

    let mut ui = chart_ui(spark().tooltip(ChartTooltip::new()).height(60.0));
    assert_eq!(images(&ui)[0].image.pixels(), images(&plain)[0].image.pixels(), "drawn the same");
    at_time(&mut ui, 0.0);
    hover(&mut ui, 780.0, 30.0);
    assert_eq!(image_node(&mut ui).value.as_deref(), Some("C, S: 2"));
    settle(&mut ui, 0.0);
    assert!(cursor_of(&ui).is_some(), "with the cursor over it");
    key(&mut ui, ravenrs_core::Key::Tab);
    key(&mut ui, ravenrs_core::Key::ArrowLeft);
    assert_eq!(image_node(&mut ui).value.as_deref(), Some("B, S: 3"));
}

#[test]
fn a_sparkline_with_no_categories_says_only_its_value() {
    let spark =
        Chart::new(ChartKind::SparkArea).series("S", [1.0, 3.0]).tooltip(ChartTooltip::new()).height(60.0);
    let mut ui = chart_ui(spark);
    hover(&mut ui, 780.0, 30.0);
    assert_eq!(image_node(&mut ui).value.as_deref(), Some("S: 3"));
}

#[test]
fn a_scatter_chart_with_a_tooltip_names_its_dots_by_their_positions() {
    let scatter = || {
        Chart::new(ChartKind::Scatter)
            .positions([1.0, 2.0, 3.0])
            .categories(["", "", ""])
            .series("Latency", [10.0, 20.0, 30.0])
    };
    let plain = chart_ui(scatter());
    let mut ui = chart_ui(scatter().tooltip(ChartTooltip::new()));
    assert_eq!(images(&ui)[0].image.pixels(), images(&plain)[0].image.pixels(), "drawn the same");
    hover(&mut ui, 10.0, 100.0);
    assert_eq!(image_node(&mut ui).value.as_deref(), Some("1, Latency: 10"));
}

#[test]
fn a_small_chart_with_a_tooltip_asks_for_no_frames_when_still() {
    let spark = Chart::new(ChartKind::Sparkline).series("S", [1.0, 3.0]).tooltip(ChartTooltip::new());
    let mut ui = chart_ui(spark.height(60.0));
    settle(&mut ui, 0.0);
    assert!(!ui.needs_redraw());
}

fn candles() -> Chart {
    Chart::new(ChartKind::Candlestick).categories(["Mon", "Tue", "Wed"]).candles([
        [100.0, 112.0, 98.0, 110.0],
        [110.0, 111.0, 101.0, 102.0],
        [102.0, 104.0, 99.0, 103.0],
    ])
}

#[test]
fn candles_are_quads_rising_as_outlines_with_an_axis_of_round_numbers() {
    let ui = chart_ui(candles());
    assert!(images(&ui).is_empty());
    let (success, danger) =
        (Theme::raven_dark().color(Semantic::Success), Theme::raven_dark().color(Semantic::Danger));
    let quads: Vec<ravenrs_core::Quad> = ui
        .scene()
        .primitives()
        .iter()
        .filter_map(|p| if let Primitive::Quad(q) = p { Some(*q) } else { None })
        .collect();
    let outlined = quads.iter().filter(|q| q.border.is_some_and(|b| b.color == success)).count();
    assert_eq!(outlined, 2, "the two that rose");
    assert_eq!(
        quads.iter().filter(|q| q.fill == danger.into() && q.rect.width() > 2.0).count(),
        1,
        "the one that fell"
    );
    assert!(glyph_runs(&ui) > 6, "categories and numbers");
}

#[test]
fn a_candle_says_its_four_prices_in_the_language_of_the_screen() {
    let mut ui = chart_ui(candles());
    hover(&mut ui, 400.0, 100.0);
    assert_eq!(image_node(&mut ui).value.as_deref(), Some("Tue, Open: 110, High: 111, Low: 101, Close: 102"));
    key(&mut ui, ravenrs_core::Key::Tab);
    key(&mut ui, ravenrs_core::Key::ArrowRight);
    assert_eq!(image_node(&mut ui).value.as_deref(), Some("Wed, Open: 102, High: 104, Low: 99, Close: 103"));

    let mut ui = chart_ui_view(Localized::new(Labels::pt_br(), candles().alt("Preço")));
    let tree = ui.access_tree();
    let node = tree.nodes.iter().find(|n| n.name.as_deref() == Some("Preço")).expect("the chart");
    let spoken = node.description.as_deref().unwrap_or_default();
    assert!(spoken.starts_with("Mon: Abertura 100, Máxima 112, Mínima 98, Fechamento 110"), "{spoken}");
}

#[test]
fn candles_fade_around_the_one_in_focus_and_ask_for_no_frames_when_still() {
    let mut ui = chart_ui(candles());
    assert!(!ui.needs_redraw());
    at_time(&mut ui, 0.0);
    move_to(&mut ui, 400.0, 100.0);
    settle(&mut ui, 0.0);
    let danger = Theme::raven_dark().color(Semantic::Danger);
    let solid = ui
        .scene()
        .primitives()
        .iter()
        .any(|p| matches!(p, Primitive::Quad(q) if q.fill == danger.into() && q.rect.width() > 2.0));
    assert!(solid, "the one in focus stays at full strength");
    let faded = ui.scene().primitives().iter().any(|p| match p {
        Primitive::Quad(q) => q.border.is_some_and(|b| b.color.a < 1.0 && b.color.g > b.color.r),
        _ => false,
    });
    assert!(faded, "the others do not");
}

/// A chart on a screen whose clock runs from the first frame, as on a platform.
fn drawing_in(view: impl View + 'static) -> Ui {
    let mut ui = Ui::new(VStack::new((view,)).align(Align::Stretch), Theme::raven_dark());
    ui.set_viewport(Size::new(800.0, 600.0), 1.0);
    ui.set_time(0.0);
    ui.update();
    ui
}

/// The clip the chart's picture is painted through, if any.
fn clip_of_picture(ui: &Ui) -> Option<Rect> {
    let mut clips = Vec::new();
    for primitive in ui.scene().primitives() {
        match primitive {
            Primitive::PushClip(rect) => clips.push(*rect),
            Primitive::PopClip => {
                clips.pop();
            }
            Primitive::Image(_) => return clips.last().copied(),
            _ => {}
        }
    }
    None
}

fn paths(ui: &Ui) -> usize {
    ui.scene().primitives().iter().filter(|p| matches!(p, Primitive::Path(_))).count()
}

#[test]
fn a_line_draws_in_from_the_left_with_its_axes_there_from_the_start() {
    let mut ui = drawing_in(line().hide_legend());
    let first = clip_of_picture(&ui).expect("a reveal").width();
    let labels = glyph_runs(&ui);
    at_time(&mut ui, 0.15);
    let later = clip_of_picture(&ui).expect("still a reveal").width();
    assert!(later > first, "{first} then {later}");
    settle(&mut ui, 0.15);
    assert_eq!(clip_of_picture(&ui), None, "whole, with no clip left");
    assert_eq!(glyph_runs(&ui), labels, "the axis labels were there all along");
    assert!(!ui.is_animating());
}

#[test]
fn bars_grow_out_of_the_zero_line_together() {
    let chart =
        Chart::new(ChartKind::Bar).categories(["A", "B", "C"]).series("S", [-4.0, 8.0, 16.0]).hide_legend();
    let color = Theme::raven_dark().color(Semantic::Chart1);
    let mut ui = drawing_in(chart);
    at_time(&mut ui, 0.1);
    let growing = bars_on_top(&ui, color);
    settle(&mut ui, 0.1);
    let whole = bars_on_top(&ui, color);
    assert_eq!(whole.len(), 3);
    // The negative one hangs from the zero line, where the others stand.
    let zero = whole[0].y();
    assert!((whole[1].max_y() - zero).abs() < 0.5);
    let share = |i: usize| {
        let bar = growing.iter().find(|r| (r.x() - whole[i].x()).abs() < 0.5);
        bar.map_or(0.0, |bar| bar.height() / whole[i].height())
    };
    assert!(share(2) > 0.0 && share(2) < 0.99, "{}", share(2));
    assert!((share(1) - share(2)).abs() < 0.02, "together: {} and {}", share(1), share(2));
    let tall = growing.iter().find(|r| (r.x() - whole[2].x()).abs() < 0.5).expect("the tallest");
    assert!((tall.max_y() - zero).abs() < 0.5, "it stands on zero");
}

#[test]
fn a_pie_sweeps_and_a_radar_spreads_over_its_rings() {
    let pie = Chart::new(ChartKind::Pie).categories(["A", "B", "C"]).series("S", [1.0, 2.0, 3.0]);
    let mut ui = drawing_in(pie);
    at_time(&mut ui, 0.05);
    assert_eq!(paths(&ui), 3, "the slices are drawn as they sweep");
    settle(&mut ui, 0.05);
    assert_eq!(paths(&ui), 0);
    assert_eq!(images(&ui).len(), 1, "and then the picture again");

    let radar =
        Chart::new(ChartKind::Radar).categories(["A", "B", "C", "D"]).series("S", [1.0, 2.0, 3.0, 4.0]);
    let mut ui = drawing_in(radar);
    at_time(&mut ui, 0.05);
    let [rings, data] = images(&ui).try_into().expect("the rings and the data apart");
    assert!(data.rect.width() < rings.rect.width() * 0.99);
    let (inner, outer) = (data.rect, rings.rect);
    let inside = inner.x() > outer.x()
        && inner.y() > outer.y()
        && inner.max_x() < outer.max_x()
        && inner.max_y() < outer.max_y();
    assert!(inside, "out of the center: {inner:?} in {outer:?}");
    settle(&mut ui, 0.05);
    assert_eq!(images(&ui).len(), 1);
}

#[test]
fn the_tooltip_waits_until_the_data_is_whole() {
    let mut ui = drawing_in(bars());
    at_time(&mut ui, 0.1);
    hover(&mut ui, 100.0, 100.0);
    assert_eq!(image_node(&mut ui).value, None);
    settle(&mut ui, 0.1);
    hover(&mut ui, 101.0, 100.0);
    assert!(image_node(&mut ui).value.is_some());
}

#[test]
fn less_motion_or_no_appear_shows_the_data_whole_at_once() {
    let mut ui = Ui::new(VStack::new((line(),)).align(Align::Stretch), Theme::raven_dark());
    ui.set_viewport(Size::new(800.0, 600.0), 1.0);
    ui.set_reduced_motion(true);
    ui.set_time(0.0);
    ui.update();
    assert_eq!(clip_of_picture(&ui), None);

    let ui = drawing_in(line().no_appear());
    assert_eq!(clip_of_picture(&ui), None);
}

#[test]
fn a_new_key_draws_the_data_in_again() {
    let period = State::new(0_u32);
    let p = period.clone();
    let mut ui = drawing_in(view_fn(move || line().appear_key(p.get())));
    settle(&mut ui, 0.0);
    assert_eq!(clip_of_picture(&ui), None);
    period.set(1);
    ui.update();
    assert!(clip_of_picture(&ui).is_some(), "it draws in again");
}

#[test]
fn a_sankey_draws_in_from_the_left() {
    let sankey = SankeyChart::new().flow("Salary", "Budget", 100.0).flow("Budget", "Rent", 60.0);
    let clip = |ui: &Ui| {
        ui.scene()
            .primitives()
            .iter()
            .find_map(|p| if let Primitive::PushClip(r) = p { Some(*r) } else { None })
    };
    let mut ui = drawing_in(sankey.clone());
    let first = clip(&ui).expect("a reveal").width();
    at_time(&mut ui, 0.2);
    assert!(clip(&ui).expect("still a reveal").width() > first);
    settle(&mut ui, 0.2);
    assert_eq!(clip(&ui), None);
    assert_eq!(clip(&drawing_in(sankey.no_appear())), None);
}

/// The heights of the bars, left to right: quads in the first series' color.
fn bar_heights(ui: &Ui) -> Vec<f32> {
    let color = Theme::raven_dark().color(Semantic::Chart1);
    let mut bars: Vec<Rect> = ui
        .scene()
        .primitives()
        .iter()
        .filter_map(|p| match p {
            Primitive::Quad(q) if q.fill == ravenrs_core::Brush::Solid(color) => Some(q.rect),
            _ => None,
        })
        .collect();
    bars.sort_by(|a, b| a.x().total_cmp(&b.x()));
    bars.iter().map(|r| r.height()).collect()
}

/// Frame by frame, since the motion clock skips a long gap as a stall.
fn run_to(ui: &mut Ui, from: f64, to: f64) {
    let mut time = from;
    while time < to {
        time = (time + 1.0 / 60.0).min(to);
        at_time(ui, time);
    }
}

fn changing_bars(values: &State<[f64; 2]>) -> Ui {
    let values = values.clone();
    let mut ui = chart_ui_view(view_fn(move || {
        Chart::new(ChartKind::Bar).categories(["A", "B"]).series("S", values.get()).animate_changes()
    }));
    // Past the first draw in.
    at_time(&mut ui, 0.0);
    run_to(&mut ui, 0.0, 2.0);
    ui
}

#[test]
fn a_chart_goes_from_its_old_data_to_the_new() {
    let values = State::new([2.0, 4.0]);
    let mut ui = changing_bars(&values);
    let before = bar_heights(&ui);
    values.set([4.0, 2.0]);
    run_to(&mut ui, 2.0, 2.15);
    let early = bar_heights(&ui);
    assert!(early[0] > before[0] && early[0] < before[1], "A rises: {before:?} {early:?}");
    assert!(early[1] < before[1] && early[1] > before[0], "B falls: {early:?}");
    // Turned back halfway, it starts from where the bars are.
    values.set([2.0, 4.0]);
    run_to(&mut ui, 2.15, 2.17);
    let turned = bar_heights(&ui);
    assert!((turned[0] - early[0]).abs() < 4.0, "no jump: {early:?} {turned:?}");
    run_to(&mut ui, 2.17, 3.5);
    assert_eq!(bar_heights(&ui), before, "and it lands where it began");
}

#[test]
fn under_reduced_motion_a_chart_changes_at_once() {
    let values = State::new([2.0, 4.0]);
    let mut ui = changing_bars(&values);
    let before = bar_heights(&ui);
    ui.set_reduced_motion(true);
    values.set([4.0, 2.0]);
    run_to(&mut ui, 2.0, 2.05);
    assert_eq!(bar_heights(&ui), [before[1], before[0]]);
}
