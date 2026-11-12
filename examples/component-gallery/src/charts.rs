//! Every pattern of the shadcn/ui chart catalog, built with nothing but the public
//! API of [`Chart`]. The names are the catalog's, to compare them one by one.

use ravenrs::prelude::*;

/// The families of the catalog, in its order.
pub const FAMILIES: [&str; 7] = ["Area", "Bar", "Line", "Pie", "Radar", "Radial", "Tooltip"];

const MONTHS: [&str; 6] = ["January", "February", "March", "April", "May", "June"];
const DESKTOP: [f64; 6] = [186.0, 305.0, 237.0, 73.0, 209.0, 214.0];
const MOBILE: [f64; 6] = [80.0, 200.0, 120.0, 190.0, 130.0, 140.0];
const OTHER: [f64; 6] = [45.0, 100.0, 150.0, 50.0, 100.0, 160.0];
const BROWSERS: [&str; 5] = ["Chrome", "Safari", "Firefox", "Edge", "Other"];
const VISITORS: [f64; 5] = [275.0, 200.0, 187.0, 173.0, 90.0];
const HEIGHT: f32 = 240.0;

fn short(month: &str) -> String {
    month.chars().take(3).collect()
}

fn months(kind: ChartKind) -> Chart {
    Chart::new(kind).categories(MONTHS).tick_format(short).height(HEIGHT)
}

fn one_series(kind: ChartKind) -> Chart {
    months(kind).series("Desktop", DESKTOP).hide_legend()
}

fn two_series(kind: ChartKind) -> Chart {
    months(kind).series("Desktop", DESKTOP).series("Mobile", MOBILE)
}

fn browsers(kind: ChartKind) -> Chart {
    Chart::new(kind).categories(BROWSERS).series("Visitors", VISITORS).height(HEIGHT)
}

fn card<V: View>(name: &str, description: &str, chart: V) -> ChartCard<V> {
    ChartCard::new(chart)
        .title(name)
        .description(description)
        .trend("Trending up by 5.2% this month", IconName::TrendingUp)
        .note("Showing total visitors for the last 6 months")
}

fn round_card(name: &str, chart: Chart) -> AnyView {
    Box::new(
        ChartCard::new(chart)
            .title(name)
            .description("January - June 2024")
            .trend("Trending up by 5.2% this month", IconName::TrendingUp)
            .note("Showing total visitors for the last 6 months")
            .centered(),
    )
}

fn area_card(name: &str, chart: Chart) -> AnyView {
    Box::new(card(name, "Showing total visitors for the last 6 months", chart))
}

fn plain_card(name: &str, chart: Chart) -> AnyView {
    Box::new(card(name, "January - June 2024", chart))
}

/// A synthetic day of visits, the same on every run.
fn visits(day: usize, seed: f64) -> f64 {
    let t = day as f64;
    (260.0 + 120.0 * (t * 0.31 + seed).sin() + 80.0 * (t * 0.77 + seed * 2.0).cos()).round()
}

fn day_name(day: usize) -> String {
    let (month, first) = if day < 30 {
        ("Apr", 1)
    } else if day < 61 {
        ("May", 31)
    } else {
        ("Jun", 62)
    };
    format!("{month} {}", day + 2 - first)
}

struct AreaInteractive;

impl View for AreaInteractive {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let range = cx.use_state(|| 0_usize);
        let days = [90_usize, 30, 7][range.get().min(2)];
        let shown = (90 - days)..90;
        let chart = Chart::new(ChartKind::Area)
            .categories(shown.clone().map(day_name))
            .series("Desktop", shown.clone().map(|d| visits(d, 0.0)))
            .series("Mobile", shown.map(|d| visits(d, 1.7)))
            .stack(ChartStack::Normal)
            .gradient()
            .tooltip(ChartTooltip::new().no_cursor())
            .height(HEIGHT);
        cx.child(
            &ChartCard::new(chart)
                .title("Area Chart - Interactive")
                .description("Showing total visitors for the selected period")
                .action(
                    Select::new(range.binding(), ["Last 3 months", "Last 30 days", "Last 7 days"])
                        .label("Period")
                        .width(160.0),
                ),
        )
    }
}

struct TabbedChart {
    kind: ChartKind,
    name: &'static str,
}

impl View for TabbedChart {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let tab = cx.use_state(|| 0_usize);
        let seed = if tab.get() == 0 { 0.0 } else { 1.7 };
        let name = if tab.get() == 0 { "Desktop" } else { "Mobile" };
        let color = if tab.get() == 0 { Semantic::Chart1 } else { Semantic::Chart2 };
        let total = |seed: f64| (0..90).map(|d| visits(d, seed)).sum::<f64>();
        let chart = Chart::new(self.kind)
            .categories((0..90).map(day_name))
            .series_color(name, cx.theme().color(color), (0..90).map(|d| visits(d, seed)))
            .hide_legend()
            .height(HEIGHT);
        cx.child(
            &ChartCard::new(chart)
                .title(self.name)
                .description("Showing total visitors for the last 3 months")
                .tabs(
                    tab.binding(),
                    [("Desktop", format!("{:.0}", total(0.0))), ("Mobile", format!("{:.0}", total(1.7)))],
                ),
        )
    }
}

struct PieInteractive;

impl View for PieInteractive {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let month = cx.use_state(|| 0_usize);
        let shown = month.get().min(4);
        let values = [186.0, 305.0, 237.0, 173.0, 209.0];
        let chart = Chart::new(ChartKind::Doughnut)
            .categories(MONTHS[..5].iter().copied())
            .series("Visitors", values)
            .active(Some(shown))
            .center_text(format!("{:.0}", values[shown]), "Visitors")
            .hide_legend()
            .height(HEIGHT);
        cx.child(
            &ChartCard::new(chart)
                .title("Pie Chart - Interactive")
                .description("January - June 2024")
                .action(
                    Select::new(month.binding(), MONTHS[..5].iter().copied()).label("Month").width(140.0),
                ),
        )
    }
}

fn area() -> Vec<AnyView> {
    let line = || ChartTooltip::new().indicator(ChartIndicator::Line);
    let stacked = || two_series(ChartKind::Area).stack(ChartStack::Normal);
    vec![
        area_card("Area Chart", one_series(ChartKind::Area).tooltip(line())),
        area_card(
            "Area Chart - Linear",
            one_series(ChartKind::Area).curve(ChartCurve::Linear).tooltip(ChartTooltip::new().hide_label()),
        ),
        area_card(
            "Area Chart - Step",
            one_series(ChartKind::Area).curve(ChartCurve::Step).tooltip(ChartTooltip::new().hide_label()),
        ),
        area_card("Area Chart - Legend", stacked().tooltip(line())),
        area_card("Area Chart - Stacked", stacked().hide_legend()),
        area_card(
            "Area Chart - Stacked Expanded",
            two_series(ChartKind::Area).series("Other", OTHER).stack(ChartStack::Expand).hide_legend(),
        ),
        area_card(
            "Area Chart - Icons",
            stacked()
                .series_icon("Desktop", IconName::Monitor)
                .series_icon("Mobile", IconName::Smartphone)
                .tooltip(line().icons()),
        ),
        area_card("Area Chart - Gradient", stacked().gradient().hide_legend()),
        area_card("Area Chart - Axes", stacked().show_value_axis().hide_legend()),
        Box::new(AreaInteractive),
    ]
}

fn bar() -> Vec<AnyView> {
    let hidden = || ChartTooltip::new().hide_label();
    vec![
        plain_card("Bar Chart", one_series(ChartKind::Bar).bar_radius(8.0).tooltip(hidden())),
        plain_card(
            "Bar Chart - Horizontal",
            one_series(ChartKind::Bar).horizontal().bar_radius(5.0).grid(ChartGrid::None).tooltip(hidden()),
        ),
        plain_card(
            "Bar Chart - Multiple",
            two_series(ChartKind::Bar)
                .hide_legend()
                .tooltip(ChartTooltip::new().indicator(ChartIndicator::Dashed)),
        ),
        plain_card(
            "Bar Chart - Stacked + Legend",
            two_series(ChartKind::Bar).stack(ChartStack::Normal).bar_radius(4.0).tooltip(hidden()),
        ),
        plain_card(
            "Bar Chart - Label",
            one_series(ChartKind::Bar).bar_radius(8.0).value_labels(ChartLabelAt::Outside).tooltip(hidden()),
        ),
        plain_card(
            "Bar Chart - Custom Label",
            one_series(ChartKind::Bar)
                .horizontal()
                .bar_radius(4.0)
                .grid(ChartGrid::None)
                .hide_category_axis()
                .category_labels(ChartLabelAt::InsideStart)
                .value_labels(ChartLabelAt::Outside)
                .tooltip(ChartTooltip::new().indicator(ChartIndicator::Line)),
        ),
        plain_card(
            "Bar Chart - Mixed",
            browsers(ChartKind::Bar)
                .horizontal()
                .vary_colors()
                .bar_radius(5.0)
                .grid(ChartGrid::None)
                .hide_legend()
                .tooltip(hidden()),
        ),
        plain_card(
            "Bar Chart - Active",
            browsers(ChartKind::Bar)
                .vary_colors()
                .active(Some(2))
                .bar_radius(8.0)
                .hide_legend()
                .tooltip(hidden()),
        ),
        plain_card(
            "Bar Chart - Negative",
            months(ChartKind::Bar)
                .series("Visitors", [186.0, 205.0, -207.0, 173.0, -209.0, 214.0])
                .hide_legend()
                .hide_category_axis()
                .negative_color(Semantic::Chart2)
                .value_labels(ChartLabelAt::Outside)
                .tooltip(hidden()),
        ),
        Box::new(TabbedChart { kind: ChartKind::Bar, name: "Bar Chart - Interactive" }),
    ]
}

fn line() -> Vec<AnyView> {
    let hidden = || ChartTooltip::new().hide_label();
    vec![
        plain_card("Line Chart", one_series(ChartKind::Line).tooltip(hidden())),
        plain_card(
            "Line Chart - Linear",
            one_series(ChartKind::Line).curve(ChartCurve::Linear).tooltip(hidden()),
        ),
        plain_card(
            "Line Chart - Step",
            one_series(ChartKind::Line).curve(ChartCurve::Step).tooltip(hidden()),
        ),
        plain_card(
            "Line Chart - Multiple",
            two_series(ChartKind::Line).hide_legend().tooltip(ChartTooltip::new().no_cursor()),
        ),
        plain_card("Line Chart - Dots", one_series(ChartKind::Line).dots().tooltip(hidden())),
        plain_card(
            "Line Chart - Custom Dots",
            one_series(ChartKind::Line).dot_icon(IconName::Star).tooltip(hidden()),
        ),
        plain_card(
            "Line Chart - Dots Colors",
            browsers(ChartKind::Line)
                .vary_colors()
                .dots()
                .hide_legend()
                .hide_category_axis()
                .tooltip(hidden()),
        ),
        plain_card(
            "Line Chart - Label",
            one_series(ChartKind::Line).dots().value_labels(ChartLabelAt::Outside).tooltip(hidden()),
        ),
        plain_card(
            "Line Chart - Custom Label",
            browsers(ChartKind::Line)
                .vary_colors()
                .dots()
                .hide_legend()
                .hide_category_axis()
                .category_labels(ChartLabelAt::Outside)
                .tooltip(ChartTooltip::new().indicator(ChartIndicator::Line)),
        ),
        Box::new(TabbedChart { kind: ChartKind::Line, name: "Line Chart - Interactive" }),
    ]
}

fn pie() -> Vec<AnyView> {
    let pie = || browsers(ChartKind::Pie).hide_legend();
    let doughnut = || browsers(ChartKind::Doughnut).hide_legend();
    let total: f64 = VISITORS.iter().sum();
    vec![
        round_card("Pie Chart", pie()),
        round_card("Pie Chart - Separator None", pie().no_separator()),
        round_card("Pie Chart - Label", pie().value_labels(ChartLabelAt::Outside)),
        round_card(
            "Pie Chart - Custom Label",
            pie().format(|value| format!("{value:.0} visits")).value_labels(ChartLabelAt::Outside),
        ),
        round_card("Pie Chart - Label List", pie().category_labels(ChartLabelAt::Inside)),
        round_card("Pie Chart - Legend", browsers(ChartKind::Pie)),
        round_card("Pie Chart - Donut", doughnut()),
        round_card("Pie Chart - Donut Active", doughnut().active(Some(0))),
        round_card("Pie Chart - Donut with Text", doughnut().center_text(format!("{total:.0}"), "Visitors")),
        round_card(
            "Pie Chart - Stacked",
            Chart::new(ChartKind::Pie)
                .categories(BROWSERS)
                .series("Desktop", VISITORS)
                .series("Mobile", [90.0, 173.0, 187.0, 200.0, 275.0])
                .concentric()
                .hole(0.2)
                .hide_legend()
                .height(HEIGHT),
        ),
        Box::new(PieInteractive),
    ]
}

fn radar() -> Vec<AnyView> {
    let one = || one_series(ChartKind::Radar);
    let two = || two_series(ChartKind::Radar);
    let figures = |month: &str| {
        let at = MONTHS.iter().position(|m| *m == month).unwrap_or(0);
        format!("{}\n{:.0}/{:.0}", short(month), DESKTOP[at], MOBILE[at])
    };
    vec![
        round_card("Radar Chart", one()),
        round_card("Radar Chart - Dots", one().dots()),
        round_card("Radar Chart - Multiple", two().hide_legend()),
        round_card("Radar Chart - Lines Only", two().lines_only().hide_legend()),
        round_card("Radar Chart - Custom Label", two().hide_legend().tick_format(figures)),
        round_card("Radar Chart - Radius Axis", two().radius_axis().hide_legend()),
        round_card(
            "Radar Chart - Grid Custom",
            one().rings(1).no_spokes().polar_grid(ChartPolarGrid::Circle),
        ),
        round_card("Radar Chart - Grid Filled", one().polar_fill().fill_opacity(0.5)),
        round_card("Radar Chart - Grid None", one().polar_grid(ChartPolarGrid::None).dots()),
        round_card("Radar Chart - Grid Circle", one().polar_grid(ChartPolarGrid::Circle).dots()),
        round_card(
            "Radar Chart - Grid Circle - No lines",
            one().polar_grid(ChartPolarGrid::Circle).no_spokes().dots(),
        ),
        round_card("Radar Chart - Grid Circle Filled", one().polar_grid(ChartPolarGrid::Circle).polar_fill()),
        round_card("Radar Chart - Legend", two()),
        round_card(
            "Radar Chart - Icons",
            two().series_icon("Desktop", IconName::Monitor).series_icon("Mobile", IconName::Smartphone),
        ),
    ]
}

fn radial() -> Vec<AnyView> {
    let rings = || browsers(ChartKind::RadialBar).hide_legend();
    let one = || {
        Chart::new(ChartKind::RadialBar)
            .categories(["Safari"])
            .series("Visitors", [200.0])
            .hide_legend()
            .tracks()
            .hole(0.72)
            .height(HEIGHT)
    };
    vec![
        round_card("Radial Chart", rings().tracks()),
        round_card(
            "Radial Chart - Label",
            rings().angles(0.0, 270.0).hole(0.3).category_labels(ChartLabelAt::InsideStart),
        ),
        round_card("Radial Chart - Grid", rings().polar_grid(ChartPolarGrid::Circle)),
        round_card("Radial Chart - Text", one().angles(0.0, 250.0).center_text("200", "Visitors")),
        round_card(
            "Radial Chart - Shape",
            one().angles(0.0, 100.0).polar_grid(ChartPolarGrid::Circle).center_text("1,260", "Visitors"),
        ),
        round_card(
            "Radial Chart - Stacked",
            Chart::new(ChartKind::RadialBar)
                .categories(["January"])
                .series("Desktop", [1260.0])
                .series("Mobile", [570.0])
                .stack(ChartStack::Normal)
                .angles(-90.0, 180.0)
                .hole(0.72)
                .center_text("1,830", "Visitors")
                .hide_legend()
                .height(HEIGHT),
        ),
    ]
}

fn tooltip() -> Vec<AnyView> {
    let days = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
    let chart = |tooltip: ChartTooltip| {
        Chart::new(ChartKind::Bar)
            .categories(days)
            .series("Running", [450.0, 380.0, 520.0, 140.0, 600.0, 480.0])
            .series("Swimming", [300.0, 420.0, 120.0, 550.0, 350.0, 400.0])
            .stack(ChartStack::Normal)
            .bar_radius(4.0)
            .tick_format(short)
            .hide_legend()
            .tooltip(tooltip)
            .height(HEIGHT)
    };
    let new = ChartTooltip::new;
    let card = |name: &str, description: &str, tooltip: ChartTooltip| {
        Box::new(ChartCard::new(chart(tooltip)).title(name).description(description))
    };
    let date = |day: &str| format!("{day}, July 2024");
    let kcal = |value: f64| format!("{value:.0} kcal");
    vec![
        card("Tooltip - Default", "Default tooltip with the indicator as a square.", new()),
        card(
            "Tooltip - Line Indicator",
            "Tooltip with a line indicator.",
            new().indicator(ChartIndicator::Line),
        ),
        card("Tooltip - No Indicator", "Tooltip with no indicator.", new().indicator(ChartIndicator::None)),
        card("Tooltip - Custom label", "Tooltip with a label of its own.", new().label("Activities")),
        card("Tooltip - Label Formatter", "Tooltip with a formatted label.", new().label_format(date)),
        card("Tooltip - No Label", "Tooltip with no label.", new().hide_label()),
        card("Tooltip - Formatter", "Tooltip with a formatted value.", new().value_format(kcal).hide_label()),
        Box::new(
            ChartCard::new(
                chart(new().icons())
                    .series_icon("Running", IconName::Zap)
                    .series_icon("Swimming", IconName::Sparkles),
            )
            .title("Tooltip - Icons")
            .description("Tooltip with an icon for each series."),
        ),
        card(
            "Tooltip - Advanced",
            "Tooltip with units and a total.",
            new().value_format(kcal).label_format(date).show_total("Total"),
        ),
    ]
}

/// The patterns of one family of the catalog.
pub fn family(index: usize) -> Vec<AnyView> {
    match index {
        0 => area(),
        1 => bar(),
        2 => line(),
        3 => pie(),
        4 => radar(),
        5 => radial(),
        _ => tooltip(),
    }
}

/// The catalog, one family at a time.
#[derive(Debug)]
pub struct ChartCatalog {
    /// How many columns fit.
    pub columns: u16,
}

impl View for ChartCatalog {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let shown = cx.use_state(|| 0_usize);
        let families = SegmentedControl::new(shown.binding(), FAMILIES).label("Chart family");
        let charts = family_grid(shown.get(), self.columns);
        cx.child(&VStack::new((families, charts)).spacing(16.0).align(Align::Stretch))
    }
}

/// One family laid out in `columns`. Each family is a new set of charts, so they
/// draw in again instead of reusing the ones that already appeared in the same
/// places.
fn family_grid(index: usize, columns: u16) -> Keyed<Grid<Vec<AnyView>>> {
    Keyed::new(index as u64, Grid::new(columns, family(index)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_catalog_has_every_pattern() {
        let sizes: Vec<usize> = (0..FAMILIES.len()).map(|i| family(i).len()).collect();
        assert_eq!(sizes, [10, 10, 10, 11, 14, 6, 9]);
        assert_eq!(sizes.iter().sum::<usize>(), 70);
    }

    #[test]
    fn another_family_draws_its_charts_in() {
        // A chart is painted as one finished picture only once it has drawn in.
        let whole = |ui: &Ui| {
            ui.scene().primitives().iter().filter(|p| matches!(p, ravenrs::core::Primitive::Image(_))).count()
        };
        let settle = |ui: &mut Ui, from: f64| {
            for step in 0..60 {
                ui.set_time(from + f64::from(step) * 0.1);
                ui.update();
            }
        };
        let shown = State::new(0_usize);
        let s = shown.clone();
        let mut ui = Ui::new(view_fn(move || family_grid(s.get(), 2)), Theme::raven_dark());
        ui.set_viewport(Size::new(1000.0, 3000.0), 1.0);
        settle(&mut ui, 0.0);
        assert!(whole(&ui) > 0, "the first family is in");

        shown.set(3);
        ui.set_time(6.0);
        ui.update();
        assert_eq!(whole(&ui), 0, "the pies start drawing in instead of showing up whole");
        settle(&mut ui, 6.0);
        assert_eq!(whole(&ui), family(3).len(), "every pie is in");
    }
}
