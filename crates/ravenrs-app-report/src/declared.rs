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

use ravenrs_app::{BusinessObject, DashboardInfo};
use ravenrs_app_ui::{App, Session};
use ravenrs_widgets::AnyView;

use crate::analytics::{TileSpec, tile_content};

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
}

impl AnalyticsPanel {
    /// A panel with no tiles yet. `name` is what its view's id starts with, as in
    /// `Sales_DashboardView`; `caption` is what people see.
    pub fn new(name: impl Into<String>, caption: impl Into<String>) -> Self {
        Self { name: name.into(), caption: caption.into(), navigation: None, icon: None, tiles: Vec::new() }
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
        self.tiles.push(TileSpec {
            title: caption.into(),
            target: entity.into(),
            kind: "number".into(),
            series: value.into(),
            ..TileSpec::default()
        });
        self
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
        self.tiles.push(TileSpec {
            title: caption.into(),
            target: entity.into(),
            kind: chart.kind().into(),
            argument: argument.into(),
            series: value.into(),
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

    /// The dashboard of the model: one control per tile, in order.
    pub fn info(&self) -> DashboardInfo {
        let mut info = DashboardInfo::new(self.name.clone(), self.caption.clone())
            .icon(self.icon.clone().unwrap_or_else(|| "layout-dashboard".into()));
        if let Some(group) = &self.navigation {
            info = info.navigation(group.clone());
        }
        for (index, tile) in self.tiles.iter().enumerate() {
            info = info.control(tile.title.clone(), self.control(index));
        }
        info
    }

    /// Registers the tiles as controls of `session`, which [`info`](Self::info) names.
    pub fn register(&self, session: &Session) {
        for (index, tile) in self.tiles.iter().enumerate() {
            let (frame, tile) = (session.frame(), tile.clone());
            // The frame links back to its session weakly, so the control keeps no cycle alive.
            session.register_control(self.control(index), move || match Session::of_frame(&frame) {
                Some(session) => {
                    // Read again when records change.
                    session.revision();
                    tile_content(&session, &tile)
                }
                None => Box::new(ravenrs_widgets::Text::new(tile.title.clone())) as AnyView,
            });
        }
    }
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
