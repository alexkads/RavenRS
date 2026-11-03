//! Panels people put together themselves and keep as records: tiles of numbers and charts over the
//! entities of the application, each read as the person can read it.

use std::collections::BTreeMap;
use std::rc::Rc;

use ravenrs_app::{
    Action, ActionCategory, Criteria, EntityInfo, EnumOption, FieldInfo, FieldKind, Module, Object, Query,
    Selection, Value, ViewController,
};
use ravenrs_app_ui::{Session, chart_view, summary_text};
use ravenrs_reactive::State;
use ravenrs_widgets::{
    AnyView, BuildCx, Button, ButtonSize, ButtonStyle, NodeId, ScrollView, Stat, Tile, TileLayout, TileSpot,
    VStack, View, ViewExt,
};

use crate::stored::{ReportWords, problem};

/// The entity of the panels people keep.
pub const ANALYTICS_DASHBOARD: &str = "AnalyticsDashboard";
/// The entity of their tiles.
pub const ANALYTICS_TILE: &str = "AnalyticsTile";
const OPEN: &str = "OpenPanel";
const TILE_COLUMNS: u16 = 6;
const TILE_ROWS: u16 = 3;
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

fn text(record: &Object, name: &str) -> String {
    match record.get(name) {
        Value::Text(text) => text,
        _ => String::new(),
    }
}

/// What a tile shows, read now, as the person can read it.
fn tile_content(session: &Session, tile: &Object) -> AnyView {
    let title = text(tile, "title");
    let target = text(tile, "target_entity");
    let Some(entity) = session.entity(&target) else {
        return problem(&title, format!("{} {target}", session.words().list_editor_needs));
    };
    let mut query = Query::all();
    let filter = text(tile, "filter");
    if !filter.trim().is_empty() {
        match Criteria::parse(&filter) {
            Ok(criteria) => query = query.filter(criteria),
            Err(error) => return problem(&title, error.to_string()),
        }
    }
    let objects = match session.frame().space().borrow().query(&target, &query) {
        Ok(objects) => objects,
        Err(error) => return problem(&title, error.to_string()),
    };
    let (kind, series) = (text(tile, "kind"), text(tile, "series"));
    if kind == "number" {
        return match summary_text(session, &entity, &objects, &series) {
            Ok(value) => Box::new(Stat::new(entity.caption.clone(), value).large()),
            Err(why) => problem(&title, why),
        };
    }
    let mut settings = BTreeMap::new();
    let chart_kind = match kind.as_str() {
        "line" => "Line",
        "area" => "Area",
        "pie" => "Pie",
        "doughnut" => "Doughnut",
        _ => "Bar",
    };
    settings.insert("Kind".to_owned(), chart_kind.to_owned());
    for (from, to) in [("argument", "Argument"), ("series", "Series"), ("interval", "Interval")] {
        let value = text(tile, from);
        if !value.trim().is_empty() {
            settings.insert(to.to_owned(), value);
        }
    }
    chart_view(session, &entity, Rc::new(objects), settings, CHART_HEIGHT)
        .unwrap_or_else(|why| problem(&title, why))
}

/// A panel on the screen.
#[derive(Clone)]
struct PanelView {
    session: Session,
    panel: Value,
    close: String,
    spots: State<Vec<TileSpot>>,
    tiles: Rc<Vec<Object>>,
}

impl PanelView {
    fn new(session: Session, panel: Value, close: String) -> Self {
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
                let rows = if text(tile, "kind") == "number" { NUMBER_ROWS } else { TILE_ROWS };
                TileSpot::new(ravenrs_app::key_text(&tile.get("id")).unwrap_or_default(), TILE_COLUMNS, rows)
            })
            .collect();
        Self { session, panel, close, spots: State::new(spots), tiles: Rc::new(tiles) }
    }
}

impl View for PanelView {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        // The records can change under it.
        self.session.revision();
        let mut tiles = TileLayout::new(self.spots.binding());
        for tile in self.tiles.iter() {
            let key = ravenrs_app::key_text(&tile.get("id")).unwrap_or_default();
            tiles = tiles.tile(Tile::new(key, text(tile, "title"), tile_content(&self.session, tile)));
        }
        let closing = self.session.clone();
        let close = Button::new(self.close.clone())
            .style(ButtonStyle::Ghost)
            .size(ButtonSize::Small)
            .on_click(move || closing.close_full());
        let _ = &self.panel;
        let content = VStack::new((close, tiles)).spacing(8.0).padding(12.0);
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
                    let shown = PanelView::new(session.clone(), record.get("id"), close.clone());
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
