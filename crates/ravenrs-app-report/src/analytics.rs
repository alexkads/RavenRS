//! Panels people put together themselves and keep as records: tiles of numbers and charts over the
//! entities of the application, each read as the person can read it.

use std::collections::BTreeMap;
use std::rc::Rc;

use ravenrs_app::{
    Action, ActionCategory, Criteria, EntityInfo, EnumOption, FieldInfo, FieldKind, Module, Object,
    Operation, Query, Selection, Value, ViewController,
};
use ravenrs_app_ui::{Session, chart_view, summary_text};
use ravenrs_layout::Align;
use ravenrs_reactive::State;
use ravenrs_style::{Semantic, TextStyle, Weight};
use ravenrs_widgets::{
    AnyView, Badge, BuildCx, Button, ButtonSize, ButtonStyle, HStack, NodeId, ScrollView, Text, Tile,
    TileLayout, TileSpot, Tone, VStack, View, ViewExt,
};

use crate::stored::{ReportWords, problem};

/// The entity of the panels people keep.
pub const ANALYTICS_DASHBOARD: &str = "AnalyticsDashboard";
/// The entity of their tiles.
pub const ANALYTICS_TILE: &str = "AnalyticsTile";
const OPEN: &str = "OpenPanel";
const TILE_COLUMNS: u16 = 4;
const NUMBER_COLUMNS: u16 = 2;
const TILE_ROWS: u16 = 2;
const NUMBER_ROWS: u16 = 1;
const CHART_HEIGHT: f32 = 240.0;

/// The entities of panels, which storage has to know. Their tiles are edited on the panel's own form,
/// so the designer is the screens every record gets.
pub fn analytics_entities() -> Vec<EntityInfo> {
    let kinds = vec![
        EnumOption::new("number", "Number"),
        EnumOption::new("bar", "Bars"),
        EnumOption::new("line", "Lines"),
        EnumOption::new("area", "Areas"),
        EnumOption::new("pie", "Pie"),
        EnumOption::new("doughnut", "Doughnut"),
    ];
    vec![
        EntityInfo::new(ANALYTICS_DASHBOARD)
            .caption("Panels")
            .navigation("Reports")
            .icon("layout-dashboard")
            .field(FieldInfo::new("id", FieldKind::Guid).key())
            .field(FieldInfo::new("name", FieldKind::Text).default_property())
            .field(
                FieldInfo::new("tiles", FieldKind::Collection(ANALYTICS_TILE.to_owned()))
                    .back_reference("dashboard_id")
                    .aggregated(),
            )
            .rule(ravenrs_app::Rule::required("name")),
        EntityInfo::new(ANALYTICS_TILE)
            .caption("Tiles")
            .field(FieldInfo::new("id", FieldKind::Guid).key())
            .field(
                FieldInfo::new("dashboard_id", FieldKind::Reference(ANALYTICS_DASHBOARD.to_owned()))
                    .caption("Panel"),
            )
            .field(FieldInfo::new("title", FieldKind::Text).default_property())
            .field(FieldInfo::new("position", FieldKind::Integer).default_value(0.0))
            .field(FieldInfo::new("target_entity", FieldKind::Text).caption("Entity"))
            .field(FieldInfo::new("kind", FieldKind::Enum(kinds)))
            .field(FieldInfo::new("argument", FieldKind::Text).caption("Along the axis"))
            .field(FieldInfo::new("series", FieldKind::Text).caption("Values, as field:Sum"))
            .field(FieldInfo::new("interval", FieldKind::Text).caption("Dates by Day, Month or Year"))
            .field(FieldInfo::new("filter", FieldKind::Text))
            .rule(ravenrs_app::Rule::required("title"))
            .rule(ravenrs_app::Rule::required("target_entity")),
    ]
}

/// Where a number stands, from the best to the worst.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum IndicatorLevel {
    /// In good standing.
    Good,
    /// Needs attention.
    Attention,
    /// Critical.
    Critical,
}

impl IndicatorLevel {
    /// What it says, in these words.
    pub fn caption(self, words: &ReportWords) -> &str {
        match self {
            Self::Good => &words.level_good,
            Self::Attention => &words.level_attention,
            Self::Critical => &words.level_critical,
        }
    }

    /// The tone it shows in.
    pub fn tone(self) -> Tone {
        match self {
            Self::Good => Tone::Success,
            Self::Attention => Tone::Warning,
            Self::Critical => Tone::Danger,
        }
    }

    /// The worst of these, if any.
    pub fn worst(levels: impl IntoIterator<Item = Self>) -> Option<Self> {
        levels.into_iter().max()
    }
}

/// How a number becomes a level: by limits, higher or lower being better, or by a rule of its own.
#[derive(Clone)]
pub struct Thresholds(Rc<dyn Fn(f64) -> Option<IndicatorLevel>>);

impl std::fmt::Debug for Thresholds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Thresholds(..)")
    }
}

impl PartialEq for Thresholds {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for Thresholds {}

impl Thresholds {
    /// Higher is better: from `good` up it is in good standing, from `attention` up it needs
    /// attention, below it is critical.
    pub fn at_least(good: f64, attention: f64) -> Self {
        Self(Rc::new(move |value| {
            Some(if value >= good {
                IndicatorLevel::Good
            } else if value >= attention {
                IndicatorLevel::Attention
            } else {
                IndicatorLevel::Critical
            })
        }))
    }

    /// Lower is better: up to `good` it is in good standing, up to `attention` it needs attention,
    /// above it is critical. `at_most(0.0, 0.0)` makes anything above zero critical.
    pub fn at_most(good: f64, attention: f64) -> Self {
        Self(Rc::new(move |value| {
            Some(if value <= good {
                IndicatorLevel::Good
            } else if value <= attention {
                IndicatorLevel::Attention
            } else {
                IndicatorLevel::Critical
            })
        }))
    }

    /// By a rule of its own, for what limits do not say; nothing means no level.
    pub fn level_with(rule: impl Fn(f64) -> Option<IndicatorLevel> + 'static) -> Self {
        Self(Rc::new(rule))
    }

    /// The level of this number.
    pub fn level(&self, value: f64) -> Option<IndicatorLevel> {
        (self.0)(value)
    }
}

/// A number worked out in code, from the person's own unit of work.
#[derive(Clone)]
pub(crate) struct Computed(pub(crate) Rc<ComputeFn>);

/// How a computed number is worked out.
pub(crate) type ComputeFn = dyn Fn(&dyn ravenrs_app::ObjectSpace) -> Result<f64, String>;

impl std::fmt::Debug for Computed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Computed(..)")
    }
}

impl PartialEq for Computed {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for Computed {}

/// What a number tile reads now, for the screen and for the printed panel alike.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Reading {
    /// Its reader may not read what it needs.
    Restricted,
    /// It cannot be worked out, and why.
    Problem(String),
    /// The number as shown, and its level when it has thresholds.
    Number(String, Option<IndicatorLevel>),
}

fn shown_number(value: f64) -> String {
    if value.fract() == 0.0 { format!("{value:.0}") } else { format!("{value:.2}") }
}

/// Reads a number tile as the person signed in can.
pub(crate) fn reading(session: &Session, tile: &TileSpec) -> Reading {
    if !readable(session, tile) {
        return Reading::Restricted;
    }
    let level = |value: Option<f64>| tile.thresholds.as_ref().and_then(|thresholds| thresholds.level(value?));
    if let Some(computed) = &tile.computed {
        let space = session.frame().space();
        let space = space.borrow();
        return match (computed.0)(&*space) {
            Ok(value) => Reading::Number(shown_number(value), level(Some(value))),
            Err(why) => Reading::Problem(why),
        };
    }
    let Some(entity) = session.entity(&tile.target) else {
        return Reading::Problem(format!("{} {}", session.words().list_editor_needs, tile.target));
    };
    let mut query = Query::all();
    if !tile.filter.trim().is_empty() {
        match Criteria::parse(&tile.filter) {
            Ok(criteria) => query = query.filter(criteria),
            Err(error) => return Reading::Problem(error.to_string()),
        }
    }
    let objects = match session.frame().space().borrow().query(&tile.target, &query) {
        Ok(objects) => objects,
        Err(error) => return Reading::Problem(error.to_string()),
    };
    match summary_text(session, &entity, &objects, &tile.series) {
        Ok(value) => Reading::Number(value, level(series_number(&entity, &objects, &tile.series))),
        Err(why) => Reading::Problem(why),
    }
}

/// The number a series gives over these records, in whole units: money comes in cents.
pub(crate) fn series_number(entity: &EntityInfo, objects: &[Object], series: &str) -> Option<f64> {
    let series = series.trim();
    if series.is_empty() {
        return Some(objects.len() as f64);
    }
    let (field, how) = series.split_once(':').unwrap_or((series, "Sum"));
    let money = entity.field_named(field).is_some_and(|field| matches!(field.kind, FieldKind::Money));
    let values: Vec<f64> = objects
        .iter()
        .filter_map(|object| object.number(field))
        .map(|number| if money { number / 100.0 } else { number })
        .collect();
    match how {
        "Count" => Some(values.len() as f64),
        "Sum" => Some(values.iter().sum()),
        "Average" if !values.is_empty() => Some(values.iter().sum::<f64>() / values.len() as f64),
        "Min" => values.iter().copied().reduce(f64::min),
        "Max" => values.iter().copied().reduce(f64::max),
        _ => None,
    }
}

/// The fields a tile reads: its series, what is along the axis and what its filter names.
fn fields_read(tile: &TileSpec) -> Vec<String> {
    let mut fields = Vec::new();
    if let Some(field) = tile.series.split(':').next().map(str::trim).filter(|field| !field.is_empty()) {
        fields.push(field.to_owned());
    }
    if !tile.argument.trim().is_empty() {
        fields.push(tile.argument.trim().to_owned());
    }
    if let Ok(criteria) = Criteria::parse(&tile.filter) {
        // The field of the record itself, the first step of a path.
        fields.extend(
            criteria.properties().into_iter().filter_map(|path| path.split('.').next().map(str::to_owned)),
        );
    }
    fields
}

/// Whether the person signed in may read all a tile needs: the entity and each field it reads.
pub(crate) fn readable(session: &Session, tile: &TileSpec) -> bool {
    let Some(security) = session.frame().security() else { return true };
    security.with(|permissions| {
        if tile.computed.is_some() {
            // What it declared it reads, as `Entity` or `Entity.field`.
            return tile.requires.iter().all(|need| match need.split_once('.') {
                Some((entity, field)) => {
                    permissions.can(Operation::Read, entity, None)
                        && permissions.can_field(Operation::Read, entity, field, None)
                }
                None => permissions.can(Operation::Read, need, None),
            });
        }
        let fields = fields_read(tile);
        permissions.can(Operation::Read, &tile.target, None)
            && fields.iter().all(|field| permissions.can_field(Operation::Read, &tile.target, field, None))
    })
}

/// What a tile is asked to show: the same fields a kept tile has, from a record or from code.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct TileSpec {
    pub(crate) title: String,
    pub(crate) target: String,
    /// `number`, `bar`, `line`, `area`, `pie` or `doughnut`.
    pub(crate) kind: String,
    pub(crate) argument: String,
    /// `field:Sum` and the like; empty counts the records.
    pub(crate) series: String,
    pub(crate) interval: String,
    pub(crate) filter: String,
    /// How its number becomes a level, if it does.
    pub(crate) thresholds: Option<Thresholds>,
    /// The group of its panel it is in, by its place among the groups.
    pub(crate) group: Option<usize>,
    /// A number worked out in code instead of over `target`.
    pub(crate) computed: Option<Computed>,
    /// What a computed number reads, as `Entity` or `Entity.field`.
    pub(crate) requires: Vec<String>,
}

impl TileSpec {
    fn of(tile: &Object) -> Self {
        Self {
            title: tile.text("title"),
            target: tile.text("target_entity"),
            kind: tile.text("kind"),
            argument: tile.text("argument"),
            series: tile.text("series"),
            interval: tile.text("interval"),
            filter: tile.text("filter"),
            ..Self::default()
        }
    }
}

/// A number with its level beside it, as a tile and a row of a group show it.
pub(crate) fn number_view(value: String, level: Option<IndicatorLevel>, words: &ReportWords) -> AnyView {
    let number = Text::new(value).style(TextStyle::Metric);
    match level {
        Some(level) => Box::new(
            HStack::new((number, Badge::new(level.caption(words)).tone(level.tone())))
                .spacing(8.0)
                .align(Align::Center),
        ),
        None => Box::new(number),
    }
}

/// What a tile shows, read now, as the person can read it.
pub(crate) fn tile_content(session: &Session, tile: &TileSpec) -> AnyView {
    let title = &tile.title;
    let words = ReportWords::from_texts(&session.texts());
    if tile.kind == "number" {
        return match reading(session, tile) {
            // A number its reader may not see is not shown as zero.
            Reading::Restricted => {
                Box::new(Text::new(words.restricted.clone()).foreground(Semantic::InkMuted))
            }
            Reading::Problem(why) => problem(title, why),
            Reading::Number(value, level) => number_view(value, level, &words),
        };
    }
    let target = &tile.target;
    let Some(entity) = session.entity(target) else {
        return problem(title, format!("{} {target}", session.words().list_editor_needs));
    };
    if !readable(session, tile) {
        return Box::new(Text::new(words.restricted.clone()).foreground(Semantic::InkMuted));
    }
    let mut query = Query::all();
    if !tile.filter.trim().is_empty() {
        match Criteria::parse(&tile.filter) {
            Ok(criteria) => query = query.filter(criteria),
            Err(error) => return problem(title, error.to_string()),
        }
    }
    let objects = match session.frame().space().borrow().query(target, &query) {
        Ok(objects) => objects,
        Err(error) => return problem(title, error.to_string()),
    };
    let mut settings = BTreeMap::new();
    let chart_kind = match tile.kind.as_str() {
        "line" => "Line",
        "area" => "Area",
        "pie" => "Pie",
        "doughnut" => "Doughnut",
        _ => "Bar",
    };
    settings.insert("Kind".to_owned(), chart_kind.to_owned());
    for (value, to) in [(&tile.argument, "Argument"), (&tile.series, "Series"), (&tile.interval, "Interval")]
    {
        if !value.trim().is_empty() {
            settings.insert(to.to_owned(), value.clone());
        }
    }
    chart_view(session, &entity, Rc::new(objects), settings, CHART_HEIGHT)
        .unwrap_or_else(|why| problem(title, why))
}

/// A panel on the screen.
#[derive(Clone)]
struct PanelView {
    session: Session,
    panel: Value,
    close: String,
    title: String,
    spots: State<Vec<TileSpot>>,
    tiles: Rc<Vec<Object>>,
}

impl PanelView {
    fn new(session: Session, panel: Value, title: String, close: String) -> Self {
        let mut tiles: Vec<Object> = session
            .frame()
            .space()
            .borrow()
            .query(ANALYTICS_TILE, &Query::all())
            .unwrap_or_default()
            .into_iter()
            .filter(|tile| tile.get("dashboard_id") == panel)
            .collect();
        tiles.sort_by_key(|tile| match tile.get("position") {
            Value::Number(position) => position as i64,
            _ => 0,
        });
        let spots = tiles
            .iter()
            .map(|tile| {
                let (columns, rows) = if tile.text("kind") == "number" {
                    (NUMBER_COLUMNS, NUMBER_ROWS)
                } else {
                    (TILE_COLUMNS, TILE_ROWS)
                };
                TileSpot::new(ravenrs_app::key_text(&tile.get("id")).unwrap_or_default(), columns, rows)
            })
            .collect();
        Self { session, panel, close, title, spots: State::new(spots), tiles: Rc::new(tiles) }
    }
}

impl View for PanelView {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        // The records can change under it.
        self.session.revision();
        let mut tiles = TileLayout::new(self.spots.binding());
        for tile in self.tiles.iter() {
            let key = ravenrs_app::key_text(&tile.get("id")).unwrap_or_default();
            tiles = tiles.tile(Tile::new(
                key,
                tile.text("title"),
                tile_content(&self.session, &TileSpec::of(tile)),
            ));
        }
        let closing = self.session.clone();
        let close = Button::new(self.close.clone())
            .style(ButtonStyle::Secondary)
            .size(ButtonSize::Small)
            .on_click(move || closing.close_full());
        let _ = &self.panel;
        let top = HStack::new((
            Text::new(self.title.clone()).style(TextStyle::Title).font_weight(Weight::SEMIBOLD).grow(1.0),
            close,
        ))
        .align(Align::Center);
        let content = VStack::new((top, tiles)).spacing(16.0).align(Align::Stretch).padding(16.0);
        cx.child(&ScrollView::new((content,)).grow(1.0))
    }
}

struct Panels {
    words: ReportWords,
}

impl ViewController for Panels {
    fn id(&self) -> &str {
        "Panels"
    }

    fn actions(&self) -> Vec<Action> {
        let close = self.words.close.clone();
        vec![
            Action::simple(OPEN, self.words.open_panel.clone())
                .category(ActionCategory::View)
                .target_entity(ANALYTICS_DASHBOARD)
                .selection(Selection::RequireSingle)
                .icon("layout-dashboard")
                .on_execute(move |cx| {
                    let session = Session::of(cx).ok_or("this action needs the screens")?;
                    let record = cx.selected.first().ok_or("this action needs a panel")?;
                    let shown =
                        PanelView::new(session.clone(), record.get("id"), record.text("name"), close.clone());
                    session.show_full(move || Box::new(shown.clone()) as AnyView);
                    Ok(())
                }),
        ]
    }
}

/// Panels people put together and keep. Storage has to know [`analytics_entities`].
#[derive(Debug, Default)]
pub struct AnalyticsModule {
    words: ReportWords,
}

impl AnalyticsModule {
    /// With these words.
    pub fn new(words: ReportWords) -> Self {
        Self { words }
    }
}

impl Module for AnalyticsModule {
    fn name(&self) -> &str {
        "Analytics"
    }

    fn entities(&self) -> Vec<EntityInfo> {
        analytics_entities()
    }

    fn view_controllers(&self) -> Vec<Box<dyn ViewController>> {
        vec![Box::new(Panels { words: self.words.clone() })]
    }
}
