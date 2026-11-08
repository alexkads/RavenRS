//! The RavenRS component gallery: every built-in component, grouped by
//! category, with live state. Runs natively and in the browser.

use ravenrs::prelude::*;

mod art;
pub mod charts;
mod pages;
pub mod samples;

fn first_of_plan() -> Date {
    Date::new(2026, 11, 1).unwrap_or_else(|| Date::from_days(0))
}

fn plan_events() -> Vec<SchedulerEvent> {
    let at = |day: i64| TimePoint::from_date(first_of_plan()).plus_days(day);
    vec![
        SchedulerEvent::new(1, 1, at(1), at(5), "Vacation").tone(Tone::Success),
        SchedulerEvent::new(2, 1, at(3), at(8), "Workshop"),
        SchedulerEvent::new(3, 2, at(2), at(6), "On call").tone(Tone::Warning),
        SchedulerEvent::new(4, 3, at(4), at(5), "Review").tone(Tone::Danger).resizable(false),
    ]
}

/// What a table of five million rows on a server would answer.
fn server_rows(with_total: bool) -> RemoteRows {
    const ROWS: usize = 5_000_000;
    RemoteRows::new(move |request, answer| {
        let end = (request.offset + request.limit).min(ROWS);
        let rows = (request.offset..end)
            .map(|n| vec![Value::Number(n as f64), Value::Text(format!("Customer {n}"))])
            .collect();
        let page = GridPage::new(rows, end < ROWS);
        answer.send(if with_total { page.total(ROWS) } else { page });
    })
}

/// Categories shown in the sidebar, in order.
const PAGES: &[(IconName, &str)] = &[
    (IconName::LayoutDashboard, "Overview"),
    (IconName::MousePointer, "Buttons"),
    (IconName::SlidersHorizontal, "Forms"),
    (IconName::PanelLeft, "Navigation"),
    (IconName::Layers, "Overlays"),
    (IconName::Table, "Data display"),
    (IconName::Database, "Data grid"),
    (IconName::Image, "Images"),
    (IconName::Calendar, "Pickers & forms"),
    (IconName::Columns3, "Layout & navigation"),
    (IconName::Rows3, "Ribbon"),
    (IconName::Bell, "Feedback"),
    (IconName::Zap, "Motion"),
    (IconName::ChartBar, "Charts"),
    (IconName::Sparkles, "Icons"),
];

/// All gallery state, created once; pages are rebuilt from it.
#[derive(Debug)]
pub struct Gallery {
    grid_scenarios: State<Vec<GridScenario>>,
    grid_remote: RemoteRows,
    grid_remote_pages: RemoteRows,
    page: State<usize>,
    clicks: State<u32>,
    name: State<String>,
    password: State<String>,
    plan: State<usize>,
    notifications: State<bool>,
    autosave: State<bool>,
    terms: State<bool>,
    newsletter: State<bool>,
    density: State<usize>,
    volume: State<f32>,
    exposure: State<f32>,
    warmth: State<f32>,
    currency: State<&'static str>,
    invoice: State<String>,
    message: State<String>,
    seats: State<i32>,
    range: State<usize>,
    tab: State<usize>,
    popover: State<bool>,
    dialog: State<bool>,
    last_action: State<String>,
    images: Vec<ImageData>,
    grid: GridData,
    grid_selection: State<std::collections::BTreeSet<usize>>,
    export_info: State<String>,
    date: State<Option<Date>>,
    plan_axis: State<TimeAxis>,
    plan_events: State<Vec<SchedulerEvent>>,
    plan_rule: State<RecurrenceRule>,
    agenda_month: State<Date>,
    country: State<Option<usize>>,
    otp: State<String>,
    email: State<String>,
    company: State<String>,
    submitted: State<bool>,
    sheet: State<bool>,
    sidebar: SidebarState,
    sidebar_collapse: State<usize>,
    sidebar_variant: State<usize>,
    sidebar_side: State<usize>,
    sidebar_picked: State<String>,
    palette: State<bool>,
    toasts: Toasts,
    text_styles: State<std::collections::BTreeSet<usize>>,
    align: State<usize>,
    pinned: State<bool>,
    page_number: State<usize>,
    motion_side: State<bool>,
    motion_dialog: State<bool>,
    motion_segment: State<usize>,
    motion_tab: State<usize>,
    motion_toggle: State<bool>,
    motion_check: State<bool>,
    motion_progress: State<f32>,
    ribbon_bold: State<bool>,
    ribbon_italic: State<bool>,
    ribbon_ruler: State<bool>,
    ribbon_grid: State<bool>,
    ribbon_table: State<bool>,
    ribbon_zoom: State<u32>,
    ribbon_search: State<String>,
    swatch: State<usize>,
}

impl Default for Gallery {
    fn default() -> Self {
        Self::new()
    }
}

impl Gallery {
    /// Creates every page's state, the sample data and the generated images.
    pub fn new() -> Self {
        Self {
            grid_scenarios: State::new(Vec::new()),
            grid_remote: server_rows(false),
            grid_remote_pages: server_rows(true),
            page: State::new(0),
            clicks: State::new(0),
            name: State::new(String::new()),
            password: State::new(String::new()),
            plan: State::new(1),
            notifications: State::new(true),
            autosave: State::new(false),
            terms: State::new(true),
            newsletter: State::new(false),
            density: State::new(1),
            swatch: State::new(0),
            volume: State::new(0.62),
            exposure: State::new(1.5),
            warmth: State::new(0.4),
            currency: State::new("BRL"),
            invoice: State::new(String::from("49,90")),
            message: State::new(String::new()),
            seats: State::new(5),
            range: State::new(1),
            tab: State::new(0),
            popover: State::new(false),
            dialog: State::new(false),
            last_action: State::new("Nothing yet".into()),
            images: art::all(),
            grid: GridData::new(sample_rows(100_000)),
            grid_selection: State::new(Default::default()),
            export_info: State::new(String::new()),
            date: State::new(Date::new(2026, 10, 31)),
            plan_axis: State::new(TimeAxis::days(first_of_plan(), 30)),
            plan_events: State::new(plan_events()),
            plan_rule: State::new(RecurrenceRule::new(Frequency::Weekly).on_weekdays([1, 4])),
            agenda_month: State::new(first_of_plan()),
            country: State::new(None),
            otp: State::new(String::new()),
            email: State::new(String::new()),
            company: State::new(String::new()),
            submitted: State::new(false),
            sheet: State::new(false),
            sidebar: SidebarState::new(true),
            sidebar_collapse: State::new(0),
            sidebar_variant: State::new(0),
            sidebar_side: State::new(0),
            sidebar_picked: State::new("Overview".into()),
            palette: State::new(false),
            toasts: Toasts::new(),
            text_styles: State::new(Default::default()),
            align: State::new(0),
            pinned: State::new(false),
            page_number: State::new(4),
            motion_side: State::new(false),
            motion_dialog: State::new(false),
            motion_segment: State::new(0),
            motion_tab: State::new(0),
            motion_toggle: State::new(true),
            motion_check: State::new(false),
            motion_progress: State::new(0.1),
            ribbon_bold: State::new(true),
            ribbon_italic: State::new(false),
            ribbon_ruler: State::new(true),
            ribbon_grid: State::new(false),
            ribbon_table: State::new(true),
            ribbon_zoom: State::new(100),
            ribbon_search: State::new(String::new()),
        }
    }
}

/// A titled card holding one demo.
fn demo<V: View + 'static>(title: &str, description: &str, content: V) -> impl View + use<V> {
    Card::new(
        VStack::new((
            VStack::new((
                Text::new(title).style(TextStyle::Headline),
                Text::new(description).style(TextStyle::Callout).foreground(Semantic::InkMuted),
            ))
            .spacing(4.0),
            content,
        ))
        .spacing(20.0)
        .align(Align::Stretch),
    )
    .corner_radius(18.0)
}

/// A sample built once, so that what it holds survives the page's rebuilds.
struct KeptSample(String);

impl View for KeptSample {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let name = self.0.clone();
        let sample = cx.use_state(move || std::rc::Rc::new(samples::sample(&name))).get();
        cx.child(sample.as_ref())
    }
}

/// One demo per named sample, for a run of small components on a page.
fn sample_demos(items: &[(&str, &str)]) -> impl View + use<> {
    let demos: Vec<AnyView> = items
        .iter()
        .map(|(name, description)| -> AnyView {
            Box::new(demo(name, description, KeptSample((*name).to_owned())))
        })
        .collect();
    VStack::new(demos).spacing(24.0).align(Align::Stretch)
}

fn page_header(title: &str, description: &str) -> impl View + use<> {
    VStack::new((
        Text::new(title).style(TextStyle::Display),
        Text::new(description).foreground(Semantic::InkMuted),
    ))
    .spacing(8.0)
}

impl Gallery {
    fn sidebar(&self) -> impl View + use<> {
        let current = self.page.get();
        let mut items: Vec<AnyView> = vec![
            Box::new(
                HStack::new((
                    Icon::new(IconName::Sparkles).size(18.0).color(Semantic::Accent),
                    Text::new("RavenRS").style(TextStyle::Headline),
                    Spacer::new(),
                    Badge::new("v27.1"),
                ))
                .spacing(12.0)
                .align(Align::Center)
                .padding((4.0, 12.0)),
            ),
            Box::new(SidebarSection::new("Components")),
        ];
        for (i, (icon, label)) in PAGES.iter().enumerate() {
            let page = self.page.clone();
            items.push(Box::new(
                SidebarItem::new(*icon, *label).selected(i == current).on_click(move || page.set(i)),
            ));
        }
        items.push(Box::new(SidebarSection::new("Resources")));
        items.push(Box::new(SidebarItem::new(IconName::FileText, "Architecture")));
        items.push(Box::new(SidebarItem::new(IconName::Palette, "Design system")));
        Sidebar::new(items)
    }

    /// Cmd/Ctrl+K: jump to any page or switch theme.
    fn palette_view(&self) -> impl View + use<> {
        let mut palette = CommandPalette::new(self.palette.binding());
        for (i, (icon, label)) in PAGES.iter().enumerate() {
            let page = self.page.clone();
            palette = palette.command(
                Command::new(format!("Go to {label}"), move || page.set(i)).icon(*icon).group("Pages"),
            );
        }
        let toasts = self.toasts.clone();
        palette.command(
            Command::new("Show a toast", move || {
                toasts.show("Hello from the palette", "Commands can run any code.", Tone::Accent)
            })
            .icon(IconName::BellRing)
            .group("Actions"),
        )
    }

    fn toolbar(&self, narrow: bool) -> AnyView {
        if narrow {
            let pages: Vec<&str> = PAGES.iter().map(|(_, label)| *label).collect();
            return Box::new(
                VStack::new((
                    Select::new(self.page.binding(), pages).width(240.0).label("Page"),
                    ThemeSwitcher::new(),
                ))
                .spacing(12.0)
                .padding((12.0, 16.0)),
            );
        }
        let (_, title) = PAGES[self.page.get().min(PAGES.len() - 1)];
        Box::new(
            HStack::new((
                Breadcrumb::new(["Components", title]),
                Spacer::new(),
                Tooltip::new(
                    IconButton::new(IconName::Search).on_click({
                        let p = self.palette.clone();
                        move || p.set(true)
                    }),
                    "Search (⌘K)",
                ),
                Tooltip::new(IconButton::new(IconName::Bell), "Notifications"),
                ThemeSwitcher::new(),
            ))
            .spacing(8.0)
            .align(Align::Center)
            .padding((16.0, 32.0)),
        )
    }

    fn page(&self, width: f32) -> AnyView {
        match self.page.get() {
            0 => Box::new(self.overview(width)),
            1 => Box::new(self.buttons(width)),
            2 => Box::new(self.forms(width)),
            3 => Box::new(self.navigation(width)),
            4 => Box::new(self.overlays(width)),
            5 => Box::new(self.data_display(width)),
            6 => Box::new(self.data_grid(width)),
            7 => Box::new(self.images(width)),
            8 => Box::new(self.pickers(width)),
            9 => Box::new(self.layout_nav(width)),
            10 => Box::new(self.ribbon_page(width)),
            11 => Box::new(self.feedback(width)),
            12 => Box::new(self.motion(width)),
            13 => Box::new(self.charts(width)),
            _ => Box::new(self.icons(width)),
        }
    }
}

/// Deterministic sample data for the data grid demo.
fn sample_rows(n: usize) -> Vec<Vec<Value>> {
    const FIRST: &[&str] = &[
        "Northwind",
        "Aurora",
        "Kestrel",
        "Lumen",
        "Vertex",
        "Harbor",
        "Nimbus",
        "Cobalt",
        "Juniper",
        "Atlas",
        "Solace",
        "Quartz",
        "Meridian",
        "Tidal",
        "Orchid",
        "Summit",
    ];
    const SECOND: &[&str] =
        &["Labs", "Studio", "Health", "Data", "Systems", "Foods", "Logistics", "Bank", "Energy", "Media"];
    const COUNTRIES: &[&str] = &[
        "Brazil",
        "Portugal",
        "Argentina",
        "Chile",
        "Mexico",
        "United States",
        "Germany",
        "Japan",
        "Canada",
        "Spain",
    ];
    const PLANS: &[&str] = &["Starter", "Team", "Business", "Enterprise"];
    const MANAGERS: &[&str] = &["Ana Lima", "Bruno Reis", "Carla Dias", "Diego Alves", "Elisa Rocha"];
    let mut seed: u64 = 0x5eed_cafe;
    let mut next = move || {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (seed >> 33) as usize
    };
    (0..n)
        .map(|i| {
            let plan = next() % PLANS.len();
            let seats = match plan {
                0 => 1 + next() % 5,
                1 => 5 + next() % 30,
                2 => 20 + next() % 150,
                _ => 100 + next() % 900,
            };
            let price = [0.0, 49.0, 89.0, 139.0][plan];
            vec![
                Value::from(i + 1),
                Value::from(format!("{} {}", FIRST[next() % FIRST.len()], SECOND[next() % SECOND.len()])),
                Value::from(COUNTRIES[next() % COUNTRIES.len()]),
                Value::from(PLANS[plan]),
                Value::from(seats),
                Value::from(seats as f64 * price),
                Value::from((40 + next() % 61) as f64),
                Value::from(next() % 10 != 0),
                Value::from(MANAGERS[next() % MANAGERS.len()]),
            ]
        })
        .collect()
}

/// How many of `max` columns fit in `width` logical pixels (cards need ~240px).
fn columns(width: f32, max: u16) -> u16 {
    ((width / 240.0).floor() as u16).clamp(1, max)
}

fn stat_card(stat: Stat) -> AnyView {
    Box::new(Card::new(stat).corner_radius(18.0))
}

impl Component for Gallery {
    fn body(&self) -> impl View {
        responsive(move |size: Size| -> AnyView {
            let narrow = size.width < 820.0;
            let content_width =
                if narrow { size.width - 32.0 } else { (size.width - 248.0 - 64.0).min(1080.0) };
            let padding = if narrow { (20.0, 16.0) } else { (28.0, 32.0) };
            let main = VStack::new((
                self.toolbar(narrow),
                Divider::horizontal(),
                // Keyed by page, so each page opens scrolled to the top with fresh local state.
                Keyed::new(
                    self.page.get() as u64,
                    ScrollView::new((PageBody(self.page(content_width)),)).padding(padding).spacing(0.0),
                ),
            ))
            .spacing(0.0)
            .align(Align::Stretch)
            .grow(1.0);
            let shell: AnyView = if narrow {
                Box::new(main)
            } else {
                Box::new(HStack::new((self.sidebar(), main)).spacing(0.0).align(Align::Stretch).grow(1.0))
            };
            Box::new(
                VStack::new((shell, self.palette_view(), Toaster::new(self.toasts.clone())))
                    .spacing(0.0)
                    .align(Align::Stretch)
                    .grow(1.0),
            )
        })
    }
}

struct PageBody(AnyView);

impl View for PageBody {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let content = cx.child(self.0.as_ref());
        cx.container(
            ravenrs::layout::LayoutStyle {
                max_width: Some(1080.0),
                align: Align::Stretch,
                ..ravenrs::layout::LayoutStyle::stack(ravenrs::layout::Axis::Vertical)
            },
            &[content],
            Paint::None,
        )
    }
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
            let options = ravenrs::web::WebOptions::new("raven-canvas");
            // `?component=Button` shows that example in a playground, for the component pages;
            // the site's `/playground/`, or `?playground`, every example, opening on `?example=`.
            let sample = requested("component").and_then(|name| super::samples::SamplePlayground::new(&name));
            let started = match sample {
                Some(sample) => ravenrs::web::start(sample, options).await,
                None if wants_playground() => {
                    let start = requested("example");
                    ravenrs::web::start(super::samples::SamplePlayground::all(start.as_deref()), options)
                        .await
                }
                None => ravenrs::web::start(super::Gallery::new(), options).await,
            };
            match started {
                Ok(app) => APP.with(|a| *a.borrow_mut() = Some(app)),
                Err(e) => web_sys::console::error_1(&e.to_string().into()),
            }
        });
    }

    fn requested(key: &str) -> Option<String> {
        let search = web_sys::window()?.location().search().ok()?;
        let params = web_sys::UrlSearchParams::new_with_str(&search).ok()?;
        params.get(key)
    }

    fn wants_playground() -> bool {
        let Some(location) = web_sys::window().map(|window| window.location()) else { return false };
        let path = location.pathname().unwrap_or_default();
        let search = location.search().unwrap_or_default();
        path.trim_end_matches('/').ends_with("/playground")
            || web_sys::UrlSearchParams::new_with_str(&search).is_ok_and(|params| params.has("playground"))
    }

    #[wasm_bindgen]
    pub fn stop() {
        APP.with(|a| a.borrow_mut().take());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_page_builds() {
        let gallery = Gallery::new();
        let page = gallery.page.clone();
        let mut ui = Ui::new(gallery, Theme::raven_dark());
        ui.set_viewport(Size::new(1280.0, 820.0), 1.0);
        for i in 0..PAGES.len() {
            page.set(i);
            ui.update();
            assert!(ui.element_count() > 40, "page {i} built only {} elements", ui.element_count());
        }
    }

    /// Where text is to the right of `left`, which leaves out a fixed sidebar.
    fn glyph_rows(ui: &Ui, left: f32) -> Vec<f32> {
        ui.scene()
            .primitives()
            .iter()
            .filter_map(|p| match p {
                ravenrs::core::Primitive::Glyphs(g) => g.glyphs.first().filter(|g| g.x >= left).map(|g| g.y),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn pages_scroll_on_desktop_and_phone_layouts() {
        for size in [Size::new(1280.0, 720.0), Size::new(375.0, 812.0)] {
            let mut ui = Ui::new(Gallery::new(), Theme::raven_dark());
            ui.set_viewport(size, 1.0);
            ui.update();
            let left = if size.width > 800.0 { 248.0 } else { 0.0 };
            let before = glyph_rows(&ui, left);
            let at = Point::new(size.width - 60.0, size.height - 100.0);
            assert!(
                ui.handle_event(&ravenrs::core::Event::Scroll {
                    position: at,
                    delta: Point::new(0.0, -200.0)
                }),
                "the page scrolls at {size:?}"
            );
            assert!(
                {
                    // What was on screen and still is moved; what comes in was not painted before.
                    let after = glyph_rows(&ui, left);
                    before.iter().any(|y| after.iter().any(|new| (new - (y - 200.0)).abs() < 0.5))
                },
                "content moved up by 200px at {size:?}"
            );
        }
    }
}
