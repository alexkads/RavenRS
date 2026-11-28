//! Panels written in code: the same tiles of numbers and charts people keep as records, declared
//! next to the entities, so an application shows its figures without seeding records by hand.
//!
//! ```
//! use ravenrs_app::BusinessObject;
//! use ravenrs_app_report::{AnalyticsApp, AnalyticsPanel};
//! use ravenrs_app_ui::App;
//!
//! #[derive(BusinessObject)]
//! struct Sale {
//!     #[field(key)]
//!     id: i64,
//!     item: String,
//!     amount: f64,
//! }
//!
//! let panel = AnalyticsPanel::new("Sales", "Vendas")
//!     .navigation("Relatórios")
//!     .number::<Sale>("Total vendido", "amount:Sum")
//!     .number::<Sale>("Vendas", "")
//!     .bar::<Sale>("Por item", "item", "amount:Sum")
//!     .pie::<Sale>("Divisão", "item", "amount:Sum");
//! let app = App::new("Loja").entity::<Sale>().analytics_panel(panel);
//! # let _ = app;
//! ```
//!
//! Each tile reads the records as the person signed in can read them, like a kept panel. The panel
//! is a [`DashboardInfo`] whose pieces are controls the session registers; without an
//! [`App`](ravenrs_app_ui::App), add [`AnalyticsPanel::info`] to the builder and call
//! [`AnalyticsPanel::register`] with the session.

use std::rc::Rc;

use ravenrs_app::{BusinessObject, DashboardInfo, ObjectSpace};
use ravenrs_app_ui::{App, Session};
use ravenrs_grid_export::document_of;
use ravenrs_layout::Align;
use ravenrs_report::Document;
use ravenrs_report_viewer::{DocumentViewer, ViewerState};
use ravenrs_style::{Semantic, TextStyle, Weight};
use ravenrs_widgets::grid::{GridColumn, GridQuery, Row, Value as Cell, to_table};
use ravenrs_widgets::{AnyView, Badge, Button, ButtonStyle, Card, HStack, Text, VStack, ViewExt};

use crate::ReportWords;
use crate::analytics::{
    Computed, IndicatorLevel, Reading, Thresholds, TileSpec, number_view, reading, tile_content,
};

/// A chart a tile of an [`AnalyticsPanel`] can draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PanelChart {
    /// Bars, one per argument.
    Bar,
    /// A line over the arguments.
    Line,
    /// A filled line.
    Area,
    /// A pie.
    Pie,
    /// A pie with a hole.
    Doughnut,
}

impl PanelChart {
    fn kind(self) -> &'static str {
        match self {
            Self::Bar => "bar",
            Self::Line => "line",
            Self::Area => "area",
            Self::Pie => "pie",
            Self::Doughnut => "doughnut",
        }
    }
}

/// A panel of numbers and charts declared in code, shown in the navigation like any view.
///
/// `value` is what a kept tile calls its series: `field:Sum`, `field:Average`, `field:Min`,
/// `field:Max` or `field:Count`, and an empty one counts the records. `argument` is the field
/// along the axis or around the pie.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnalyticsPanel {
    name: String,
    caption: String,
    navigation: Option<String>,
    icon: Option<String>,
    tiles: Vec<TileSpec>,
    groups: Vec<PanelGroup>,
}

/// An area of a panel: its numbers in one card, under the worst of their levels.
#[derive(Clone, Debug, PartialEq, Eq)]
struct PanelGroup {
    caption: String,
    note: Option<String>,
    view: Option<(String, String)>,
}

impl AnalyticsPanel {
    /// A panel with no tiles yet. `name` is what its view's id starts with, as in
    /// `Sales_DashboardView`; `caption` is what people see.
    pub fn new(name: impl Into<String>, caption: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            caption: caption.into(),
            navigation: None,
            icon: None,
            tiles: Vec::new(),
            groups: Vec::new(),
        }
    }

    /// The group it goes under in the navigation.
    pub fn navigation(mut self, group: impl Into<String>) -> Self {
        self.navigation = Some(group.into());
        self
    }

    /// The name of its icon; `layout-dashboard` when not given.
    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// A number over the records of `T`.
    pub fn number<T: BusinessObject>(self, caption: impl Into<String>, value: impl Into<String>) -> Self {
        self.number_in(T::entity_info().name, caption, value)
    }

    /// Bars over the records of `T`, one per `argument`.
    pub fn bar<T: BusinessObject>(
        self,
        caption: impl Into<String>,
        argument: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        self.chart_in(T::entity_info().name, PanelChart::Bar, caption, argument, value)
    }

    /// A line over the records of `T`.
    pub fn line<T: BusinessObject>(
        self,
        caption: impl Into<String>,
        argument: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        self.chart_in(T::entity_info().name, PanelChart::Line, caption, argument, value)
    }

    /// A pie over the records of `T`, one slice per `argument`.
    pub fn pie<T: BusinessObject>(
        self,
        caption: impl Into<String>,
        argument: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        self.chart_in(T::entity_info().name, PanelChart::Pie, caption, argument, value)
    }

    /// A doughnut over the records of `T`.
    pub fn doughnut<T: BusinessObject>(
        self,
        caption: impl Into<String>,
        argument: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        self.chart_in(T::entity_info().name, PanelChart::Doughnut, caption, argument, value)
    }

    /// A number over the records of an entity known by name, for entities declared at run time.
    pub fn number_in(
        mut self,
        entity: impl Into<String>,
        caption: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        let group = self.current_group();
        self.tiles.push(TileSpec {
            title: caption.into(),
            target: entity.into(),
            kind: "number".into(),
            series: value.into(),
            group,
            ..TileSpec::default()
        });
        self
    }

    /// A number worked out in code, such as one that crosses two entities, read from the person's
    /// own unit of work. `requires` says what it reads, as `Entity` or `Entity.field`: if the
    /// person may not read all of it, the tile says it is restricted instead of a number.
    pub fn number_with(
        mut self,
        caption: impl Into<String>,
        requires: &[&str],
        compute: impl Fn(&dyn ObjectSpace) -> Result<f64, String> + 'static,
    ) -> Self {
        let group = self.current_group();
        self.tiles.push(TileSpec {
            title: caption.into(),
            kind: "number".into(),
            group,
            computed: Some(Computed(Rc::new(compute))),
            requires: requires.iter().map(|need| (*need).to_owned()).collect(),
            ..TileSpec::default()
        });
        self
    }

    /// The tiles declared from here on go in a group with this caption: one card with each number
    /// in a row, under the worst of their levels.
    pub fn group(mut self, caption: impl Into<String>) -> Self {
        self.groups.push(PanelGroup { caption: caption.into(), note: None, view: None });
        self
    }

    /// The rule of the last group, written for whoever reads it.
    pub fn group_note(mut self, note: impl Into<String>) -> Self {
        if let Some(group) = self.groups.last_mut() {
            group.note = Some(note.into());
        }
        self
    }

    /// A button on the last group that opens this view, with this word.
    pub fn group_view(mut self, view: impl Into<String>, verb: impl Into<String>) -> Self {
        if let Some(group) = self.groups.last_mut() {
            group.view = Some((view.into(), verb.into()));
        }
        self
    }

    fn current_group(&self) -> Option<usize> {
        self.groups.len().checked_sub(1)
    }

    /// A chart over the records of an entity known by name.
    pub fn chart_in(
        mut self,
        entity: impl Into<String>,
        chart: PanelChart,
        caption: impl Into<String>,
        argument: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        let group = self.current_group();
        self.tiles.push(TileSpec {
            title: caption.into(),
            target: entity.into(),
            kind: chart.kind().into(),
            argument: argument.into(),
            series: value.into(),
            group,
            ..TileSpec::default()
        });
        self
    }

    /// Only the records that match `criteria`, in the language of filters, for the last tile.
    pub fn filtered(mut self, criteria: impl Into<String>) -> Self {
        if let Some(tile) = self.tiles.last_mut() {
            tile.filter = criteria.into();
        }
        self
    }

    /// The number of the last tile shows its level by these thresholds: in good standing,
    /// attention or critical, with the tone of each. A tile whose reader may not see the number
    /// says it is restricted instead, with no level.
    pub fn level(mut self, thresholds: Thresholds) -> Self {
        if let Some(tile) = self.tiles.last_mut() {
            tile.thresholds = Some(thresholds);
        }
        self
    }

    /// Dates along the axis of the last tile grouped by `Day`, `Month` or `Year`.
    pub fn by(mut self, interval: impl Into<String>) -> Self {
        if let Some(tile) = self.tiles.last_mut() {
            tile.interval = interval.into();
        }
        self
    }

    fn control(&self, index: usize) -> String {
        format!("{}.tile{index}", self.name)
    }

    fn group_control(&self, index: usize) -> String {
        format!("{}.group{index}", self.name)
    }

    fn print_control(&self) -> String {
        format!("{}.print", self.name)
    }

    /// The dashboard of the model: a control per group, one per tile outside a group, in order,
    /// and the one that prints the panel.
    pub fn info(&self) -> DashboardInfo {
        let mut info = DashboardInfo::new(self.name.clone(), self.caption.clone())
            .icon(self.icon.clone().unwrap_or_else(|| "layout-dashboard".into()));
        if let Some(group) = &self.navigation {
            info = info.navigation(group.clone());
        }
        let mut shown = Vec::new();
        for (index, tile) in self.tiles.iter().enumerate() {
            match tile.group {
                Some(group) if shown.contains(&group) => {}
                Some(group) => {
                    shown.push(group);
                    info = info.control(self.groups[group].caption.clone(), self.group_control(group));
                }
                None => info = info.control(tile.title.clone(), self.control(index)),
            }
        }
        info.control(self.caption.clone(), self.print_control())
    }

    /// Registers the tiles, the groups and the printing as controls of `session`, which
    /// [`info`](Self::info) names.
    pub fn register(&self, session: &Session) {
        for (index, tile) in self.tiles.iter().enumerate().filter(|(_, tile)| tile.group.is_none()) {
            let (frame, tile) = (session.frame(), tile.clone());
            // The frame links back to its session weakly, so the control keeps no cycle alive.
            session.register_control(self.control(index), move || match Session::of_frame(&frame) {
                Some(session) => {
                    // Read again when records change.
                    session.revision();
                    tile_content(&session, &tile)
                }
                None => Box::new(Text::new(tile.title.clone())) as AnyView,
            });
        }
        for (index, group) in self.groups.iter().enumerate() {
            let tiles: Vec<TileSpec> =
                self.tiles.iter().filter(|tile| tile.group == Some(index)).cloned().collect();
            let (frame, group) = (session.frame(), group.clone());
            session.register_control(self.group_control(index), move || match Session::of_frame(&frame) {
                Some(session) => {
                    session.revision();
                    group_card(&session, &group, &tiles)
                }
                None => Box::new(Text::new(group.caption.clone())) as AnyView,
            });
        }
        let (frame, panel) = (session.frame(), self.clone());
        session.register_control(self.print_control(), move || {
            let words = Session::of_frame(&frame)
                .map(|session| ReportWords::from_texts(&session.texts()))
                .unwrap_or_default();
            let (frame, panel) = (frame.clone(), panel.clone());
            Box::new(Button::new(words.print_panel.clone()).style(ButtonStyle::Secondary).on_click(
                move || {
                    if let Some(session) = Session::of_frame(&frame) {
                        let document = Rc::new(panel.document(&session));
                        let state = ViewerState::new();
                        session.present(panel.caption.clone(), move || {
                            Box::new(DocumentViewer::new(document.clone(), &state).height(640.0)) as AnyView
                        });
                    }
                },
            )) as AnyView
        });
    }

    /// The panel on paper: a row per number, with its group, value and level, read as the screen
    /// reads it. A number its reader may not see says it is restricted, with no value.
    pub fn document(&self, session: &Session) -> Document {
        let words = ReportWords::from_texts(&session.texts());
        let rows: Vec<Row> = self
            .tiles
            .iter()
            .filter(|tile| tile.kind == "number")
            .map(|tile| {
                let group = tile.group.map(|group| self.groups[group].caption.clone()).unwrap_or_default();
                let (value, level) = match reading(session, tile) {
                    Reading::Restricted => (words.restricted.clone(), String::new()),
                    Reading::Problem(why) => (why, String::new()),
                    Reading::Number(value, level) => {
                        (value, level.map(|level| level.caption(&words).to_owned()).unwrap_or_default())
                    }
                };
                [group, tile.title.clone(), value, level].into_iter().map(Cell::Text).collect()
            })
            .collect();
        let columns = [&words.panel_group, &words.panel_indicator, &words.panel_value, &words.panel_level]
            .map(|title| GridColumn::text(title.clone()));
        let every: Vec<usize> = (0..rows.len()).collect();
        let table = to_table(&self.caption, "", &columns, &rows, &GridQuery::default(), &every);
        document_of(&table)
    }
}

/// A group as a card: its caption with the worst level, a row per number, the rule and the button
/// to its view. All restricted, it says only that.
fn group_card(session: &Session, group: &PanelGroup, tiles: &[TileSpec]) -> AnyView {
    let words = ReportWords::from_texts(&session.texts());
    let readings: Vec<(&TileSpec, Reading)> =
        tiles.iter().map(|tile| (tile, reading(session, tile))).collect();
    let title = Text::new(group.caption.clone()).style(TextStyle::Headline).font_weight(Weight::SEMIBOLD);
    if readings.iter().all(|(_, reading)| *reading == Reading::Restricted) {
        let restricted = Text::new(words.restricted.clone()).foreground(Semantic::InkMuted);
        return Box::new(Card::new(VStack::new((title, restricted)).spacing(8.0).align(Align::Start)));
    }
    let worst = IndicatorLevel::worst(readings.iter().filter_map(|(_, reading)| match reading {
        Reading::Number(_, level) => *level,
        _ => None,
    }));
    let mut top: Vec<AnyView> = vec![Box::new(title.grow(1.0))];
    if let Some(level) = worst {
        top.push(Box::new(Badge::new(level.caption(&words)).tone(level.tone())));
    }
    let mut lines: Vec<AnyView> = vec![Box::new(HStack::new(top).spacing(8.0).align(Align::Center))];
    for (tile, reading) in readings {
        let value: AnyView = match reading {
            Reading::Restricted => {
                Box::new(Text::new(words.restricted.clone()).foreground(Semantic::InkMuted))
            }
            Reading::Problem(why) => Box::new(Text::new(why).foreground(Semantic::Danger)),
            Reading::Number(value, level) => number_view(value, level, &words),
        };
        lines.push(Box::new(
            HStack::new(vec![Box::new(Text::new(tile.title.clone()).grow(1.0)) as AnyView, value])
                .spacing(8.0)
                .align(Align::Center),
        ));
    }
    if let Some(note) = &group.note {
        lines.push(Box::new(Text::new(note.clone()).foreground(Semantic::InkMuted)));
    }
    if let Some((view, verb)) = &group.view {
        let (frame, view) = (session.frame(), view.clone());
        lines.push(Box::new(Button::new(verb.clone()).style(ButtonStyle::Secondary).on_click(move || {
            if let Some(session) = Session::of_frame(&frame) {
                session.open_view(&view);
            }
        })));
    }
    Box::new(Card::new(VStack::new(lines).spacing(8.0).align(Align::Stretch)))
}

/// Adds [`AnalyticsPanel`]s to an [`App`].
pub trait AnalyticsApp {
    /// Shows `panel` in the navigation, with its tiles read from the records.
    #[must_use]
    fn analytics_panel(self, panel: AnalyticsPanel) -> Self;
}

impl AnalyticsApp for App {
    fn analytics_panel(self, panel: AnalyticsPanel) -> Self {
        self.dashboard(panel.info()).with_session(move |session| panel.register(session))
    }
}
