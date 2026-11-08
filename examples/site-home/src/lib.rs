//! The RavenRS home page, built only from RavenRS components and drawn by RavenRS.
//!
//! On the web it starts from `web_entry` with WebGL2, so it opens in every browser
//! and phone; `cargo run -p site-home` shows the same page in a window.

use std::rc::Rc;

use ravenrs::core::Key;
use ravenrs::prelude::*;
use ravenrs_report::{Document, ReportEngine, samples};
use ravenrs_report_viewer::PageFan;

/// Where the rest of the site lives, relative to the home page.
const DEMO: &str = "demo/";
const DESIGNER: &str = "designer/";
const COMPONENTS: &str = "components/";
const API: &str = "api/ravenrs/";
const GITHUB: &str = "https://github.com/alexkads/RavenRS";
const GUIDE: &str = "https://github.com/alexkads/RavenRS/blob/master/docs/GETTING_STARTED.md";
/// Height of the bar pinned at the top; anchors land below it.
const BAR: f32 = 64.0;

/// The reports the fan shows, rendered once by the real engine.
#[derive(Debug)]
struct Reports {
    customers: Document,
    invoice: Document,
    dashboard: Document,
    sales: Document,
}

impl Reports {
    fn render() -> Self {
        let mut engine = ReportEngine::new();
        let mut run = |report: ravenrs_report::Report| {
            let data = report.embedded_data().cloned().unwrap_or(serde_json::Value::Null);
            engine.render(&report, &data)
        };
        let customers = run(samples::customer_list());
        let dashboard = run(samples::sales_dashboard());
        let sales = run(samples::sales_by_region(300));
        let invoice = engine.render(&samples::invoice(), &samples::invoice_data());
        Self { customers, invoice, dashboard, sales }
    }
}

/// The whole page.
#[derive(Debug)]
pub struct Home {
    reports: Rc<Reports>,
    palette: State<bool>,
    platform: State<usize>,
    clicks: State<u32>,
}

impl Default for Home {
    fn default() -> Self {
        Self::new()
    }
}

impl Home {
    /// The page, with its sample reports rendered.
    pub fn new() -> Self {
        Self {
            reports: Rc::new(Reports::render()),
            palette: State::new(false),
            platform: State::new(0),
            clicks: State::new(0),
        }
    }
}

impl View for Home {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let links = cx.links();
        let open = self.palette.clone();
        cx.shortcut(Key::Character("k".into()), true, Rc::new(move || open.set(!open.get())));
        let (reports, palette, platform, clicks) =
            (self.reports.clone(), self.palette.clone(), self.platform.clone(), self.clicks.clone());
        let page = responsive(move |size| -> AnyView {
            let narrow = size.width < 760.0;

            Box::new(ZStack::new((
                ScrollView::new((
                    // Room for the bar, which floats over the page.
                    Spacer::new().frame(None, Some(BAR)),
                    hero(size, narrow, links.clone()),
                    Anchor::new("demo", demo(links.clone())).offset(BAR),
                    Anchor::new("pipeline", pipeline(narrow, clicks.clone())).offset(BAR),
                    Anchor::new("components", components(narrow)).offset(BAR),
                    Anchor::new("reports", reporting(reports.clone(), narrow, links.clone())).offset(BAR),
                    Anchor::new("platforms", platforms(narrow, platform.clone())).offset(BAR),
                    Anchor::new("roadmap", roadmap(narrow)).offset(BAR),
                    closing(links.clone()),
                ))
                .spacing(0.0)
                .height(size.height),
                nav(narrow, palette.clone()),
                command_palette(palette.clone(), links.clone()),
            )))
        });
        cx.child(&page)
    }
}

/// A column that gives its children its whole width, so their text wraps.
fn stack<C: ViewSeq>(children: C) -> VStack<C> {
    VStack::new(children).align(Align::Stretch)
}

/// A handler that opens `url` through the links service.
fn go(links: &Links, url: &'static str) -> impl Fn() + 'static {
    let links = links.clone();
    move || links.open(url)
}

/// A section's column: centered, at most 1160 pixels wide, with room on the sides.
fn column<V: View + 'static>(content: V) -> impl View {
    HStack::new((
        Spacer::new(),
        stack((content,)).frame(None, None).max_width(1160.0).grow(1.0),
        Spacer::new(),
    ))
    .padding(Insets::symmetric(0.0, 20.0))
}

fn section<V: View + 'static>(content: V) -> impl View {
    stack((column(content),)).padding(Insets::symmetric(96.0, 0.0))
}

fn kicker(text: &str) -> Badge {
    Badge::new(text).tone(Tone::Accent)
}

fn heading(text: &str) -> Text {
    Text::new(text).style(TextStyle::Display)
}

fn lede(text: &str) -> Text {
    Text::new(text).foreground(Semantic::InkMuted)
}

fn nav(narrow: bool, palette: State<bool>) -> impl View {
    let brand = HStack::new((
        Icon::new(IconName::Layers).color(Semantic::Accent),
        Text::new("RavenRS").style(TextStyle::Headline),
        Badge::new("v27.1"),
    ))
    .spacing(10.0)
    .align(Align::Center);
    let links: AnyView = if narrow {
        Box::new(Spacer::new())
    } else {
        Box::new(
            HStack::new((
                Link::new("Components", COMPONENTS).muted(),
                Link::new("Reports", "#reports").muted(),
                Link::new("Docs", GUIDE).muted(),
                Link::new("API", API).muted(),
            ))
            .spacing(4.0),
        )
    };
    let search = Button::new(if narrow { "" } else { "Search" })
        .icon(IconName::Search)
        .style(ButtonStyle::Secondary)
        .size(ButtonSize::Small)
        .shortcut("⌘K")
        .on_click(move || palette.set(true));
    column(
        HStack::new((brand, Spacer::new(), links, search, Link::new("GitHub", GITHUB).icon(IconName::Star)))
            .spacing(8.0)
            .align(Align::Center)
            .frame(None, Some(BAR)),
    )
    .background(Semantic::Canvas)
}

/// The app screen the floor of squares stands up into: a sidebar, cards and a chart.
fn screen(u: f32, v: f32, time: f64) -> Color {
    let theme = Theme::raven_dark();
    let c = |s: Semantic| theme.color(s);
    let inside = |a: f32, b: f32, top: f32, bottom: f32| u >= a && u <= b && v >= top && v <= bottom;
    if v < 0.075 {
        return if inside(0.012, 0.028, 0.02, 0.06) {
            c(Semantic::Danger)
        } else if inside(0.036, 0.052, 0.02, 0.06) {
            c(Semantic::Warning)
        } else if inside(0.06, 0.076, 0.02, 0.06) {
            c(Semantic::Success)
        } else {
            c(Semantic::Surface)
        };
    }
    if u < 0.2 {
        if inside(0.02, 0.18, 0.13, 0.2) {
            return if inside(0.03, 0.05, 0.14, 0.19) { c(Semantic::Accent) } else { c(Semantic::Control) };
        }
        let row = [0.25, 0.33, 0.41, 0.49].iter().any(|t| inside(0.03, 0.15, *t, t + 0.03));
        return if row { c(Semantic::Border) } else { c(Semantic::Surface) };
    }
    if inside(0.24, 0.48, 0.11, 0.15) {
        return c(Semantic::Ink);
    }
    if inside(0.24, 0.42, 0.18, 0.2) {
        return c(Semantic::InkMuted);
    }
    if inside(0.84, 0.96, 0.11, 0.18) {
        return c(Semantic::Accent);
    }
    for (a, b) in [(0.24, 0.47), (0.495, 0.725), (0.75, 0.96)] {
        if inside(a, b, 0.25, 0.43) {
            return if inside(a + 0.02, a + 0.14, 0.32, 0.37) {
                c(Semantic::Ink)
            } else if inside(a + 0.02, a + 0.1, 0.28, 0.3) {
                c(Semantic::InkMuted)
            } else if inside(a + 0.02, a + 0.07, 0.39, 0.41) {
                c(Semantic::Success)
            } else {
                c(Semantic::SurfaceElevated)
            };
        }
    }
    if inside(0.24, 0.7, 0.48, 0.94) {
        if inside(0.27, 0.67, 0.53, 0.9) && (u - 0.27) % 0.04 < 0.028 {
            let bar = ((u - 0.27) / 0.04).floor();
            let top = 0.9 - 0.37 * (0.35 + 0.55 * (0.5 + 0.5 * (bar * 1.7 + time as f32 * 1.2).sin()));
            if v >= top {
                return c(Semantic::Accent);
            }
        }
        return c(Semantic::SurfaceElevated);
    }
    if inside(0.725, 0.96, 0.48, 0.94) {
        if inside(0.745, 0.94, 0.83, 0.9) {
            return c(Semantic::Accent);
        }
        return c(Semantic::SurfaceElevated);
    }
    c(Semantic::Canvas)
}

fn hero(size: Size, narrow: bool, links: Links) -> impl View {
    let height = (size.height - BAR).clamp(560.0, 920.0);
    let (columns, rows) = if narrow { (72, 40) } else { (128, 72) };
    let field = QuadField::new(columns, rows, screen).height(height);
    let words = stack((
        HStack::new((
            kicker("Free and open source"),
            lede("160+ components · desktop and WebGL from one codebase"),
        ))
        .spacing(10.0)
        .align(Align::Center),
        Text::new("Beautiful native interfaces, powered by Rust.").style(TextStyle::Display),
        lede(
            "Write your UI once, declaratively. RavenRS draws every pixel on the GPU: in a native window on \
             Windows, macOS and Linux, and in the browser, as this page is.",
        ),
        HStack::new((
            Button::new("Try the live demo")
                .icon(IconName::Play)
                .size(ButtonSize::Large)
                .on_click(go(&links, DEMO)),
            Button::new("Open the report designer")
                .icon(IconName::FileText)
                .style(ButtonStyle::Secondary)
                .size(ButtonSize::Large)
                .on_click(go(&links, DESIGNER)),
        ))
        .spacing(12.0)
        .wrap(),
        CodeBlock::new("cargo add ravenrs"),
    ))
    .spacing(20.0)
    .max_width(820.0)
    .padding(Insets::symmetric(if narrow { 28.0 } else { 72.0 }, 20.0));
    let fading = ScrollLinked::new(words, ScrollLink::fade_out().between(0.0, 0.3));
    let caption = ScrollLinked::new(
        Badge::new("Not a video: every square is drawn by RavenRS, every frame").tone(Tone::Accent),
        ScrollLink::fade_in().between(0.8, 1.0),
    );
    ScrollScene::new(ZStack::new((
        field,
        stack((column(fading), Spacer::new(), stack((caption,)).align(Align::Center).padding(28.0)))
            .frame(None, Some(height)),
    )))
    .length(height)
    .top(BAR)
}

fn demo(links: Links) -> impl View {
    section(
        Card::new(
            stack((
                kicker("Live demo"),
                heading("Every component, running in your browser."),
                lede(
                    "This page is a RavenRS app. The gallery is a bigger one: every component, the themes, a \
                     100,000-row data grid, typing, dragging and the keyboard.",
                ),
                Button::new("Open the gallery").icon(IconName::Play).on_click(go(&links, DEMO)),
            ))
            .spacing(14.0),
        )
        .padding(32.0),
    )
}

const STEPS: [(&str, &str); 6] = [
    ("State", "A click changes a value. RavenRS knows exactly which views read it."),
    ("View", "Plain Rust builders describe the tree. No macros required."),
    ("Layout", "Flex and grid solve sizes and positions for what changed."),
    ("Text", "Shaped, wrapped and drawn from a glyph atlas at your pixel density."),
    ("Paint", "One SDF shader draws corners, borders, gradients and shadows."),
    ("Present", "The GPU presents the frame: Metal, DX12, Vulkan, WebGPU or WebGL2."),
];

const CODE: [&str; 6] = [
    "let count = State::new(0);\nlet inc = count.clone();\n\n// Readers are tracked. Only they redraw.\nButton::new(\"Add one\")\n    .icon(IconName::Plus)\n    .on_click(move || inc.update(|n| *n += 1))",
    "stack((\n    Text::new(\"Welcome\").style(TextStyle::Display),\n    Card::new(Text::bound(&count, |n| format!(\"Clicks: {n}\"))),\n    Button::new(\"Add one\").on_click(on_inc),\n))\n.spacing(16.0)",
    "// Flex and grid, solved once per change.\nHStack::new((sidebar, content.grow(1.0), inspector))\n    .spacing(16.0)\n    .padding(32.0)",
    "// Parley shapes and wraps; swash rasterizes into a GPU atlas.\nText::new(\"RavenRS\").style(TextStyle::Title)",
    "// Corners, borders, gradients and soft shadows.\nCard::new(body)\n    .corner_radius(24.0)\n    .shadow(Shadow::soft())",
    "// Only the window and input glue differ.\nfn main() {\n    ravenrs::run(\"Counter\", app);   // desktop\n}\n// scripts/build-web.sh counter  → browser",
];

/// The share of the scene each step holds, with a short cross-fade into the next.
fn step_link(i: usize, opacity_off: f32) -> ScrollLink {
    let n = STEPS.len() as f32;
    let (from, to) = (i as f32 / n, (i + 1) as f32 / n);
    let off = Effect::IDENTITY.with_opacity(opacity_off);
    let mut stops = vec![];
    if i > 0 {
        stops.push((from - 0.04, off));
    }
    stops.push((from + 0.02, Effect::IDENTITY));
    stops.push((to - 0.02, Effect::IDENTITY));
    if i + 1 < STEPS.len() {
        stops.push((to + 0.04, off));
    }
    ScrollLink::keyframes(stops)
}

fn step_visual(i: usize, clicks: &State<u32>) -> AnyView {
    match i {
        0 => {
            let (shown, inc) = (clicks.clone(), clicks.clone());
            Box::new(
                Card::new(
                    HStack::new((
                        stack((
                            Text::new("CLICKS").style(TextStyle::Caption).foreground(Semantic::InkMuted),
                            Text::dynamic(move || shown.get().to_string()).style(TextStyle::Metric),
                        ))
                        .spacing(4.0),
                        Button::new("Add one").icon(IconName::Plus).on_click(move || inc.update(|n| *n += 1)),
                    ))
                    .spacing(24.0)
                    .align(Align::Center),
                )
                .padding(24.0),
            )
        }
        1 => Box::new(
            stack((
                Badge::new("VStack").tone(Tone::Accent),
                HStack::new((Badge::new("Text"), Badge::new("Card"), Badge::new("Button"))).spacing(10.0),
                Badge::new("Text::bound(count)"),
            ))
            .spacing(14.0)
            .align(Align::Center),
        ),
        2 => Box::new(
            HStack::new((
                Card::new(Text::new("Fixed")).padding(20.0),
                Card::new(Text::new("Grows")).padding(20.0).grow(1.0),
                Card::new(Text::new("Hugs")).padding(20.0),
            ))
            .spacing(10.0),
        ),
        3 => Box::new(Text::new("RavenRS · Inter · 16 px").style(TextStyle::Metric)),
        4 => Box::new(Card::new(Stat::new("Revenue", "R$ 128.450").delta("+12.4%", true)).padding(24.0)),
        _ => Box::new(
            stack((
                ProgressBar::new(1.0).label("Metal · macOS"),
                ProgressBar::new(1.0).label("DX12 · Windows"),
                ProgressBar::new(1.0).label("Vulkan · Linux"),
                ProgressBar::new(1.0).label("WebGPU or WebGL2 · browser").tone(Tone::Success),
            ))
            .spacing(12.0)
            .align(Align::Stretch),
        ),
    }
}

fn pipeline(narrow: bool, clicks: State<u32>) -> impl View {
    let steps: Vec<AnyView> = STEPS
        .iter()
        .enumerate()
        .map(|(i, (title, text))| {
            let title =
                HStack::new((Badge::new((i + 1).to_string()), Text::new(*title).style(TextStyle::Headline)))
                    .spacing(10.0)
                    .align(Align::Center);
            // On a phone the panel and the list share the screen: titles only.
            let row: AnyView =
                if narrow { Box::new(title) } else { Box::new(stack((title, lede(text))).spacing(4.0)) };
            Box::new(ScrollLinked::new(row, step_link(i, 0.35))) as AnyView
        })
        .collect();
    let panes: Vec<AnyView> = (0..STEPS.len())
        .map(|i| {
            let pane = stack((
                stack((step_visual(i, &clicks),)).align(Align::Center).frame(None, Some(240.0)),
                CodeBlock::new(CODE[i]),
            ))
            .spacing(16.0)
            .align(Align::Stretch);
            Box::new(ScrollLinked::new(pane, step_link(i, 0.0))) as AnyView
        })
        .collect();
    let text = stack((
        kicker("From state to pixels"),
        heading("One frame, six steps, no webview."),
        lede("Scroll: the section holds while a click travels through the engine."),
        stack(steps).spacing(14.0),
    ))
    .spacing(14.0);
    let panel = Card::new(ZStack::new(panes)).padding(20.0);
    let body: AnyView = if narrow {
        Box::new(stack((panel, text)).spacing(20.0))
    } else {
        Box::new(HStack::new((text.grow(1.0), panel.grow(1.0))).spacing(48.0).align(Align::Start))
    };
    ScrollScene::new(stack((column(body),)).padding(Insets::symmetric(72.0, 0.0)))
        .length(if narrow { 1800.0 } else { 2400.0 })
        .top(BAR)
}

fn tile<V: View + 'static>(name: &str, content: V) -> impl View {
    Card::new(
        stack((
            content,
            Text::new(name.to_uppercase()).style(TextStyle::Caption).foreground(Semantic::InkMuted),
        ))
        .spacing(14.0),
    )
    .padding(18.0)
    .frame(Some(300.0), None)
}

fn components(narrow: bool) -> impl View {
    let (on, off, checked) = (State::new(true), State::new(false), State::new(true));
    let (email, level, segment) =
        (State::new(String::from("you@example.com")), State::new(0.62_f32), State::new(0));
    let first = HStack::new((
        tile(
            "Button",
            HStack::new((Button::new("Save changes"), Button::new("Cancel").style(ButtonStyle::Secondary)))
                .spacing(8.0),
        ),
        tile(
            "Toggle",
            stack((
                Toggle::new(on.binding()).label("Sync on Wi-Fi"),
                Toggle::new(off.binding()).label("Weekly digest"),
            ))
            .spacing(10.0),
        ),
        tile(
            "Badge",
            HStack::new((
                Badge::new("Overdue").tone(Tone::Danger),
                Badge::new("Due today").tone(Tone::Warning),
                Badge::new("Paid").tone(Tone::Success),
            ))
            .spacing(6.0),
        ),
        tile("TextField", TextField::new(email.binding()).icon(IconName::Mail)),
        tile("Stat", Stat::new("Active users", "8,214").delta("+3.1%", true)),
        tile(
            "Avatar",
            HStack::new((Avatar::new("Ana Lima"), Avatar::new("Bruno Reis"), Avatar::new("Carla Dias")))
                .spacing(6.0),
        ),
    ))
    .spacing(16.0);
    let second = HStack::new((
        tile("SegmentedControl", SegmentedControl::new(segment.binding(), ["Day", "Week", "Month"])),
        tile("Checkbox", Checkbox::new("Email me receipts", checked.binding())),
        tile("Slider", Slider::new(level.binding()).label("Volume")),
        tile("ProgressBar", ProgressBar::new(0.7).label("Exporting PDF").percent()),
        tile(
            "Kbd",
            HStack::new((Kbd::new("⌘"), Kbd::new("K"), Text::new("Search").foreground(Semantic::InkMuted)))
                .spacing(6.0),
        ),
        tile(
            "BarChart",
            BarChart::new([("Mon", 42.0), ("Tue", 58.0), ("Wed", 51.0), ("Thu", 73.0)]).height(90.0),
        ),
    ))
    .spacing(16.0);
    let stat = |label: &str, value: f64, suffix: &'static str, note: &str| {
        Card::new(
            stack((
                Text::new(label.to_uppercase()).style(TextStyle::Caption).foreground(Semantic::InkMuted),
                CountUp::new(value).format(move |v| format!("{}{suffix}", group(v))),
                Badge::new(note),
            ))
            .spacing(8.0),
        )
        .padding(24.0)
    };
    stack((
        section(stack((
            kicker("Components"),
            heading("160+ components, already designed and tested."),
            lede("Every component reads the same tokens, works with the keyboard and screen readers, and runs unchanged on desktop and the web. These are live: try them."),
        ))
        .spacing(14.0)),
        Ticker::new(first).speed(30.0),
        Ticker::new(second).speed(34.0).reverse(),
        section(Grid::new(
            if narrow { 2 } else { 4 },
            (
                stat("Components", 160.0, "+", "One token set"),
                stat("DataGrid rows", 100_000.0, "", "Virtualized"),
                stat("Report pages", 305.0, "", "In about 0.2 s"),
                stat("Platforms", 4.0, "", "One renderer"),
            ),
        )
        .spacing(16.0)),
    ))
    .spacing(16.0)
}

/// `1234567.0` as `1,234,567`.
fn group(v: f64) -> String {
    let digits = (v.round() as u64).to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn reporting(reports: Rc<Reports>, narrow: bool, links: Links) -> impl View {
    let last = reports.sales.pages.len().saturating_sub(1);
    let fan = PageFan::new()
        .width(if narrow { 200.0 } else { 260.0 })
        .spread(if narrow { 20.0 } else { 56.0 })
        .page(&reports.customers, 0, "Customer list", DESIGNER)
        .page(&reports.invoice, 0, "Invoice", DESIGNER)
        .page(&reports.dashboard, 0, "Sales dashboard", DESIGNER)
        .page(&reports.sales, last, "Sales by region, last page", DESIGNER)
        .page(&reports.sales, 1, "Sales by region, page 2", DESIGNER)
        .page(&reports.sales, 0, "Sales by region, page 1", DESIGNER);
    let words = stack((
        kicker("RavenRS Reporting"),
        heading("From data to a finished report in under a minute."),
        lede(
            "A report designer your users run in the browser, and an engine that lays out 10,000 rows in 305 pages \
             in about 0.2 seconds. These pages are its real output, drawn by RavenRS as you look.",
        ),
        HStack::new((
            Button::new("Open the report designer").on_click(go(&links, DESIGNER)),
            Button::new("Feature matrix")
                .style(ButtonStyle::Secondary)
                .on_click(go(&links, "https://github.com/alexkads/RavenRS/blob/master/docs/REPORTING-PARITY.md")),
        ))
        .spacing(12.0)
        .wrap(),
    ))
    .spacing(14.0);
    let feature = |title: &str, text: &str| {
        Card::new(stack((Text::new(title).style(TextStyle::Headline), lede(text))).spacing(6.0)).padding(20.0)
    };
    let top: AnyView = if narrow {
        Box::new(stack((words, stack((fan,)).align(Align::Center))).spacing(32.0))
    } else {
        Box::new(HStack::new((words.grow(1.0), fan)).spacing(40.0).align(Align::Center))
    };
    section(
        stack((
            top,
            Grid::new(
                if narrow { 1 } else { 4 },
                (
                    feature(
                        "Bands and groups",
                        "Headers and footers, nested groups with subtotals, master-detail.",
                    ),
                    feature("Expressions", "Sum([Total]), Iif(...), calculated fields and parameters."),
                    feature("Reports are data", "One JSON file holds the layout and its data sources."),
                    feature("Same engine everywhere", "Desktop, browser and server produce the same pages."),
                ),
            )
            .spacing(16.0),
        ))
        .spacing(40.0),
    )
}

const PLATFORMS: [&str; 4] = ["macOS", "Windows", "Linux", "Web"];

fn platforms(narrow: bool, platform: State<usize>) -> impl View {
    let chosen = platform.clone();
    let title_bar = view_fn(move || -> AnyView {
        let name = PLATFORMS[chosen.get().min(3)];
        let title = Text::new(format!("Finance — RavenRS on {name}"))
            .style(TextStyle::Callout)
            .foreground(Semantic::InkMuted);
        Box::new(
            HStack::new((
                if chosen.get() == 0 {
                    Box::new(
                        HStack::new((
                            Dot::new().tone(Tone::Danger),
                            Dot::new().tone(Tone::Warning),
                            Dot::new().tone(Tone::Success),
                        ))
                        .spacing(6.0),
                    ) as AnyView
                } else {
                    Box::new(Icon::new(IconName::Layers).color(Semantic::Accent)) as AnyView
                },
                Spacer::new(),
                title,
                Spacer::new(),
            ))
            .spacing(8.0)
            .align(Align::Center)
            .padding(12.0),
        )
    });
    let sidebar = Sidebar::new((SidebarGroup::new(
        "Day to day",
        (
            SidebarItem::new(IconName::LayoutDashboard, "Overview").selected(true),
            SidebarItem::new(IconName::CreditCard, "Accounts").badge(Badge::new("2").tone(Tone::Danger)),
            SidebarItem::new(IconName::Target, "Budgets"),
            SidebarItem::new(IconName::FileText, "Statements"),
        ),
    ),));
    let main = stack((
        Text::new("Overview").style(TextStyle::Display),
        Grid::new(
            2,
            (
                Card::new(Stat::new("Balance", "R$ 18.240").delta("+4.2%", true)).padding(20.0),
                Card::new(Stat::new("Spent this month", "R$ 4.917").delta("-0.4%", false)).padding(20.0),
            ),
        )
        .spacing(16.0),
        Card::new(
            BarChart::new([
                ("Mon", 48.0),
                ("Tue", 70.0),
                ("Wed", 62.0),
                ("Thu", 96.0),
                ("Fri", 86.0),
                ("Sat", 34.0),
            ])
            .height(150.0),
        )
        .padding(20.0),
    ))
    .spacing(16.0)
    .padding(24.0)
    .grow(1.0);
    let window_body: AnyView =
        if narrow { Box::new(main) } else { Box::new(HStack::new((sidebar, main)).align(Align::Stretch)) };
    section(
        stack((
            kicker("Runs everywhere"),
            heading("The same pixels on every platform, in three themes."),
            lede("Switch the platform, then the theme: the whole page redraws in it, because it is a RavenRS app."),
            HStack::new((SegmentedControl::new(platform.binding(), PLATFORMS), Spacer::new(), ThemeSwitcher::new()))
                .spacing(12.0)
                .wrap(),
            Card::new(stack((title_bar, Divider::horizontal(), window_body)).align(Align::Stretch)),
        ))
        .spacing(16.0),
    )
}

fn roadmap(narrow: bool) -> impl View {
    let phase = |n: u8, title: &str, text: &str, state: (&str, Tone)| {
        let row = HStack::new((
            Text::new(format!("PHASE {n}"))
                .style(TextStyle::Caption)
                .foreground(Semantic::InkMuted)
                .frame(Some(90.0), None),
            stack((Text::new(title).style(TextStyle::Headline), lede(text))).spacing(2.0).grow(1.0),
            Badge::new(state.0).tone(state.1),
        ))
        .spacing(16.0)
        .align(Align::Center);
        stack((row, Divider::horizontal())).spacing(14.0).align(Align::Stretch)
    };
    let done = ("Done", Tone::Success);
    let next = ("In progress", Tone::Accent);
    let _ = narrow;
    section(
        stack((
            kicker("Roadmap"),
            heading("Built in the open, one phase at a time."),
            Card::new(
                stack((
                    ProgressBar::new(3.0 / 7.0).label("3 of 7 phases done"),
                    phase(
                        1,
                        "Foundation",
                        "Workspace, checks, wgpu on desktop and the web, SDF shapes, text.",
                        done,
                    ),
                    phase(
                        2,
                        "Components",
                        "160+ components, a Ribbon, a scheduler, docking and a 100k-row DataGrid.",
                        done,
                    ),
                    phase(
                        3,
                        "Quality and release",
                        "crates.io, WAI-ARIA keyboard patterns, golden images, budgets.",
                        done,
                    ),
                    phase(
                        4,
                        "Accessibility and platform",
                        "Screen readers, clipboard and IME on the web are done.",
                        next,
                    ),
                    phase(
                        5,
                        "Reporting",
                        "Engine, web designer, PDF, XLSX and CSV export, scheduled delivery.",
                        next,
                    ),
                    phase(
                        6,
                        "Polish",
                        "Vector chart lines, Glass backdrop blur, rebuilding only what changed.",
                        ("Planned", Tone::Neutral),
                    ),
                    phase(
                        7,
                        "Applications",
                        "Business apps from a model, local AI, an MCP server, training films.",
                        next,
                    ),
                ))
                .spacing(14.0)
                .align(Align::Stretch),
            )
            .padding(24.0),
        ))
        .spacing(16.0),
    )
}

fn closing(links: Links) -> impl View {
    stack((
        section(
            stack((
                Text::new("Build the interface you would be proud to ship.").style(TextStyle::Display),
                lede("Open source under MIT or Apache-2.0. Issues, ideas and pull requests are welcome."),
                HStack::new((
                    Button::new("Read the getting started guide")
                        .size(ButtonSize::Large)
                        .on_click(go(&links, GUIDE)),
                    Button::new("Browse the API docs")
                        .style(ButtonStyle::Secondary)
                        .size(ButtonSize::Large)
                        .on_click(go(&links, API)),
                ))
                .spacing(12.0)
                .wrap(),
            ))
            .spacing(18.0)
            .align(Align::Center),
        ),
        Divider::horizontal(),
        column(
            HStack::new((
                lede("RavenRS · MIT OR Apache-2.0"),
                Spacer::new(),
                lede("Drawn by RavenRS with WebGL2. Set in Inter."),
            ))
            .padding(Insets::symmetric(24.0, 0.0)),
        ),
    ))
    .align(Align::Stretch)
}

fn command_palette(open: State<bool>, links: Links) -> impl View {
    CommandPalette::new(open.binding())
        .command(Command::new("Getting started", go(&links, GUIDE)).icon(IconName::BookOpen).group("Guides"))
        .command(Command::new("Component gallery", go(&links, DEMO)).icon(IconName::Play).group("Try it"))
        .command(
            Command::new("Report designer", go(&links, DESIGNER)).icon(IconName::FileText).group("Try it"),
        )
        .command(
            Command::new("All components", go(&links, COMPONENTS))
                .icon(IconName::LayoutDashboard)
                .group("Reference"),
        )
        .command(Command::new("API docs", go(&links, API)).icon(IconName::Code).group("Reference"))
        .command(
            Command::new("Reports on this page", go(&links, "#reports"))
                .icon(IconName::FileText)
                .group("This page"),
        )
        .command(Command::new("Roadmap", go(&links, "#roadmap")).icon(IconName::Target).group("This page"))
        .command(Command::new("GitHub repository", go(&links, GITHUB)).icon(IconName::Star).group("Source"))
}

#[cfg(target_arch = "wasm32")]
mod web_entry {
    use std::cell::RefCell;

    use wasm_bindgen::prelude::*;

    thread_local! {
        static APP: RefCell<Option<ravenrs::web::WebApp>> = const { RefCell::new(None) };
    }

    #[wasm_bindgen(start)]
    pub fn start() {
        console_error_panic_hook::set_once();
        wasm_bindgen_futures::spawn_local(async {
            // WebGL2 opens in every browser and phone, with or without WebGPU.
            let options =
                ravenrs::web::WebOptions::new("raven-canvas").backend(ravenrs::web::WebBackend::WebGl2);
            match ravenrs::web::start(super::Home::new(), options).await {
                Ok(app) => APP.with(|a| *a.borrow_mut() = Some(app)),
                Err(e) => web_sys_log(&e.to_string()),
            }
        });
    }

    fn web_sys_log(message: &str) {
        let _ = message;
    }
}
