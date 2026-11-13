//! Reports kept as records: people create them in the designer, on the data of an entity, and whoever
//! can read one runs it over what they can read.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use ravenrs_app::{
    Action, ActionCategory, EntityInfo, FieldInfo, FieldKind, Message, MessageKind, Module, Object,
    ObjectSpace, Operation, Query, Selection, TargetScope, Value, ViewController, ViewKind,
};
use ravenrs_app_i18n::{Language, Texts};
use ravenrs_app_ui::Session;
use ravenrs_reactive::State;
use ravenrs_report::wizard::{self, ColumnWords, WizardOptions};
use ravenrs_report::{DataSource, DataSourceKind, Document, RenderOptions, Report, ReportEngine};
use ravenrs_report_designer::Designer;
use ravenrs_report_export::ExportFormat;
use ravenrs_report_viewer::{DocumentViewer, ParameterPanel, ParameterValues, ViewerState};
use ravenrs_widgets::{
    Alert, AnyView, BuildCx, Button, ButtonSize, ButtonStyle, Date, NodeId, Tone, VStack, View, ViewExt,
};

/// The entity of the reports people keep.
pub const REPORT_DEFINITION: &str = "ReportDefinition";
const SHOW_IN_REPORT: &str = "ShowInReport";
const SHOW: &str = "ShowReport";
const DESIGN: &str = "DesignReport";
/// How much of an entity the designer has at hand for its preview.
const SAMPLE_ROWS: usize = 200;
const VIEWER_HEIGHT: f32 = 720.0;

/// Their words in Brazilian Portuguese, as a difference for `translation`.
pub const PT_BR: &str = ravenrs_app_i18n::model::REPORTS_PT_BR;

/// What the screens of stored reports say.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct ReportWords {
    /// Shows a report.
    pub show: String,
    /// Opens a report in the designer.
    pub design: String,
    /// Shows the records of a list in a report.
    pub show_in_report: String,
    /// Keeps the layout.
    pub save: String,
    /// Leaves.
    pub close: String,
    /// It was kept.
    pub saved: String,
    /// There is nothing to show this list in.
    pub none_for_list: String,
    /// The report names an entity there is not.
    pub unknown_entity: String,
    /// What is kept does not read as a report.
    pub broken_layout: String,
    /// Asks which report.
    pub pick_report: String,
    /// Opens a panel.
    pub open_panel: String,
}

impl Default for ReportWords {
    fn default() -> Self {
        Self::from_texts(&Texts::new(Language::English))
    }
}

impl ReportWords {
    /// The ones that come with the framework, in this language.
    pub fn of(language: Language) -> Self {
        Self::from_texts(&Texts::new(language))
    }

    /// The texts of a catalog, in its language. What it does not have comes from English.
    pub fn from_texts(texts: &Texts) -> Self {
        Self {
            show: texts.get("report.show"),
            design: texts.get("report.design"),
            show_in_report: texts.get("report.show_in_report"),
            save: texts.get("report.save"),
            close: texts.get("report.close"),
            saved: texts.get("report.saved"),
            none_for_list: texts.get("report.none_for_list"),
            unknown_entity: texts.get("report.unknown_entity"),
            broken_layout: texts.get("report.broken_layout"),
            pick_report: texts.get("report.pick_report"),
            open_panel: texts.get("report.open_panel"),
        }
    }

    /// In Brazilian Portuguese.
    pub fn pt_br() -> Self {
        Self::from_texts(&Texts::new(Language::BrazilianPortuguese))
    }
}

/// The entity of stored reports, which storage has to know. Who can see or change one goes by the
/// permissions on it, as for any other record.
pub fn report_entity() -> EntityInfo {
    EntityInfo::new(REPORT_DEFINITION)
        .caption("Reports")
        .navigation("Reports")
        .icon("file-text")
        .field(FieldInfo::new("id", FieldKind::Guid).key())
        .field(FieldInfo::new("name", FieldKind::Text).default_property())
        .field(FieldInfo::new("target_entity", FieldKind::Text).caption("Entity"))
        .field(FieldInfo::new("description", FieldKind::LongText))
        .field(FieldInfo::new("layout", FieldKind::LongText).in_list(false).in_detail(false))
        .rule(ravenrs_app::Rule::required("name"))
        .rule(ravenrs_app::Rule::required("target_entity"))
}

/// A value as a report reads it. What a person cannot read comes empty, like everywhere else.
fn json_of(
    field: &FieldInfo,
    value: &Value,
    names: &BTreeMap<String, BTreeMap<String, String>>,
) -> serde_json::Value {
    use serde_json::{Value as Json, json};
    match (&field.kind, value) {
        (_, Value::Null) => Json::Null,
        (FieldKind::Reference(target), value) => {
            let key = ravenrs_app::key_text(value).unwrap_or_default();
            let name = names.get(target).and_then(|all| all.get(&key)).cloned();
            json!(name.unwrap_or(key))
        }
        (FieldKind::Money, Value::Number(cents)) => json!(cents / 100.0),
        (FieldKind::Enum(options), Value::Text(code)) => {
            json!(options.iter().find(|o| o.code == *code).map_or(code.as_str(), |o| o.caption.as_str()))
        }
        (FieldKind::Image | FieldKind::File, _) => Json::Null,
        (FieldKind::Date, Value::Date(when)) => {
            let date = Date::from_days(when.days);
            json!(format!("{:04}-{:02}-{:02}", date.year(), date.month(), date.day()))
        }
        (_, Value::Date(when)) => {
            let date = Date::from_days(when.days);
            let (hour, minute) = (when.seconds / 3600, when.seconds % 3600 / 60);
            json!(format!(
                "{:04}-{:02}-{:02}T{hour:02}:{minute:02}:00",
                date.year(),
                date.month(),
                date.day()
            ))
        }
        (_, Value::Number(number)) => json!(number),
        (_, Value::Bool(on)) => json!(on),
        (_, Value::Text(text)) => json!(text),
        _ => Json::Null,
    }
}

fn plain_fields(entity: &EntityInfo) -> impl Iterator<Item = &FieldInfo> {
    entity.fields.iter().filter(|field| {
        !field.secret && !matches!(field.kind, FieldKind::Collection(_) | FieldKind::Image | FieldKind::File)
    })
}

/// What names each record of the entities this one points to, as this person can read them.
fn names_for(session: &Session, entity: &EntityInfo) -> BTreeMap<String, BTreeMap<String, String>> {
    let mut all = BTreeMap::new();
    for field in &entity.fields {
        let FieldKind::Reference(target) = &field.kind else { continue };
        let Some(other) = session.entity(target) else { continue };
        let (Some(key), Some(name)) = (other.key_field(), other.default_property()) else { continue };
        let found = session.frame().space().borrow().query(target, &Query::all()).unwrap_or_default();
        let names = found
            .iter()
            .filter_map(|object| {
                let key = ravenrs_app::key_text(&object.get(&key.name))?;
                match object.get(&name.name) {
                    Value::Text(text) => Some((key, text)),
                    _ => None,
                }
            })
            .collect();
        all.insert(target.clone(), names);
    }
    all
}

/// Records as the rows a report reads.
pub(crate) fn rows_of(session: &Session, entity: &EntityInfo, objects: &[Object]) -> serde_json::Value {
    let names = names_for(session, entity);
    serde_json::Value::Array(
        objects
            .iter()
            .map(|object| {
                let mut row = serde_json::Map::new();
                for field in plain_fields(entity) {
                    row.insert(field.name.clone(), json_of(field, &object.get(&field.name), &names));
                }
                serde_json::Value::Object(row)
            })
            .collect(),
    )
}

/// A report with these rows as its data, and no way of its own to get any: what a layout says about
/// addresses or queries does not run here, so all it can show is what the person can read.
fn with_rows(mut report: Report, entity: &EntityInfo, rows: serde_json::Value) -> Report {
    report.data_sources =
        vec![DataSource { name: entity.name.clone(), kind: DataSourceKind::Json { data: rows } }];
    report.data_source = Some(entity.name.clone());
    report.data_member = None;
    report
}

/// A first layout for an entity: a table of what its list shows, with money added up.
fn first_layout(session: &Session, name: &str, entity: &EntityInfo, rows: serde_json::Value) -> Report {
    let words = session.words();
    let columns: Vec<_> = plain_fields(entity).filter(|field| field.in_list).collect();
    let options = WizardOptions {
        title: name.to_owned(),
        culture: if words.decimal == ',' { "pt-BR".into() } else { "en-US".into() },
        columns: columns
            .iter()
            .map(|field| wizard::FieldInfo {
                path: field.name.clone(),
                kind: match field.kind {
                    FieldKind::Integer | FieldKind::Decimal(_) | FieldKind::Money => {
                        wizard::FieldKind::Number
                    }
                    FieldKind::Boolean => wizard::FieldKind::Boolean,
                    FieldKind::Date | FieldKind::DateTime => wizard::FieldKind::Date,
                    _ => wizard::FieldKind::Text,
                },
            })
            .collect(),
        totals: columns
            .iter()
            .filter(|field| matches!(field.kind, FieldKind::Money))
            .map(|field| field.name.clone())
            .collect(),
        ..WizardOptions::default()
    };
    let column_words: Vec<ColumnWords> = columns
        .iter()
        .map(|field| ColumnWords {
            caption: Some(field.caption.clone()),
            format: matches!(field.kind, FieldKind::Money).then(|| format!("{}#,##0.00", words.money_prefix)),
        })
        .collect();
    let source = DataSource { name: entity.name.clone(), kind: DataSourceKind::Json { data: rows } };
    wizard::table_report_with(source, &options, &column_words)
}

/// What is kept: the layout alone, and no one's data.
fn layout_to_keep(mut report: Report) -> String {
    for source in &mut report.data_sources {
        source.kind = DataSourceKind::Json { data: serde_json::Value::Null };
    }
    report.to_json()
}

/// The stored report on this record, whatever shape it is in, or what is wrong with it.
pub(crate) fn report_of(
    session: &Session,
    record: &Object,
    words: &ReportWords,
) -> Result<(Report, EntityInfo), String> {
    let text = |name: &str| match record.get(name) {
        Value::Text(text) => text,
        _ => String::new(),
    };
    let target = text("target_entity");
    let entity = session.entity(&target).ok_or_else(|| format!("{} {target}", words.unknown_entity))?;
    let layout = text("layout");
    if layout.trim().is_empty() {
        return Ok((
            first_layout(session, &text("name"), &entity, serde_json::Value::Array(Vec::new())),
            entity,
        ));
    }
    let report = Report::from_json(&layout).map_err(|error| format!("{} {error}", words.broken_layout))?;
    Ok((report, entity))
}

pub(crate) fn every_record(session: &Session, entity: &str, limit: Option<usize>) -> Vec<Object> {
    let query = match limit {
        Some(limit) => Query::all().page(0, limit),
        None => Query::all(),
    };
    session.frame().space().borrow().query(entity, &query).unwrap_or_default()
}

/// Shows a report over these records, in place of the view.
fn show_report(
    session: &Session,
    report: Report,
    entity: &EntityInfo,
    objects: &[Object],
    words: &ReportWords,
) {
    let rows = rows_of(session, entity, objects);
    let shown = ReportView::new(session.clone(), with_rows(report, entity, rows), words.clone());
    session.show_full(move || Box::new(shown.clone()) as AnyView);
}

/// Opens the designer on a stored report, with what the entity holds at hand.
pub(crate) fn design_report(session: &Session, record: &Object, words: &ReportWords) -> Result<(), String> {
    let (report, entity) = report_of(session, record, words)?;
    let sample = every_record(session, &entity.name, Some(SAMPLE_ROWS));
    let rows = rows_of(session, &entity, &sample);
    let name = match record.get("name") {
        Value::Text(name) => name,
        _ => String::new(),
    };
    let report = if record.get("layout") == Value::Null {
        first_layout(session, &name, &entity, rows)
    } else {
        with_rows(report, &entity, rows)
    };
    let (saving, closing, told) = (session.clone(), session.clone(), words.saved.clone());
    let (key, entity_name) = (record.get("id"), REPORT_DEFINITION);
    let designer = Rc::new(
        Designer::embedded(
            report,
            words.save.clone(),
            words.close.clone(),
            move |report| {
                let space = saving.frame().space();
                let mut space = space.borrow_mut();
                let result = (|| {
                    let mut stored = space
                        .find(entity_name, &key)?
                        .ok_or_else(|| ravenrs_app::SpaceError::Conflict(told.clone()))?;
                    stored.set("layout", layout_to_keep(report));
                    space.save(stored)?;
                    space.commit()
                })();
                if let Err(error) = result {
                    space.rollback();
                    drop(space);
                    saving.show_message(Message::new(error.to_string(), MessageKind::Error));
                } else {
                    drop(space);
                    saving.show_message(Message::new(told.clone(), MessageKind::Success));
                    saving.records_changed();
                }
            },
            move || closing.close_full(),
        )
        .theme_menu(session.theme_menu()),
    );
    session.show_full(move || Box::new(SharedDesigner(designer.clone())) as AnyView);
    Ok(())
}

struct SharedDesigner(Rc<Designer>);

impl View for SharedDesigner {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        self.0.build(cx)
    }
}

/// The last render, by the parameters it was for.
type Rendered = (String, Rc<Document>);

/// A report on the screen: its parameters, its pages and getting it out as a file.
#[derive(Clone)]
struct ReportView {
    session: Session,
    report: Rc<Report>,
    words: ReportWords,
    applied: State<ParameterValues>,
    state: Rc<ViewerState>,
    engine: Rc<RefCell<ReportEngine>>,
    rendered: Rc<RefCell<Option<Rendered>>>,
}

impl ReportView {
    fn new(session: Session, report: Report, words: ReportWords) -> Self {
        Self {
            session,
            report: Rc::new(report),
            words,
            applied: State::new(ParameterValues::new()),
            state: Rc::new(ViewerState::new()),
            engine: Rc::new(RefCell::new(ReportEngine::new())),
            rendered: Rc::default(),
        }
    }

    fn document(&self) -> Rc<Document> {
        let parameters = self.applied.get();
        let mut sorted: Vec<_> = parameters.iter().collect();
        sorted.sort_by_key(|(name, _)| *name);
        let key = format!("{sorted:?}");
        if let Some((known, document)) = self.rendered.borrow().as_ref()
            && *known == key
        {
            return document.clone();
        }
        let data = self.report.embedded_data().cloned().unwrap_or(serde_json::Value::Null);
        let options = RenderOptions { parameters: parameters.clone(), ..RenderOptions::default() };
        let document = Rc::new(self.engine.borrow_mut().render_with(&self.report, &data, &options));
        *self.rendered.borrow_mut() = Some((key, document.clone()));
        document
    }
}

impl View for ReportView {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let document = self.document();
        let picker = cx.file_picker();
        let (session, closing) = (self.session.clone(), self.session.clone());
        let trailing_document = document.clone();
        let close = self.words.close.clone();
        let viewer = DocumentViewer::new(document, &self.state).toolbar_trailing(move || {
            let export = |label: &str, format: ExportFormat| -> AnyView {
                let (session, picker, document) =
                    (session.clone(), picker.clone(), trailing_document.clone());
                Box::new(
                    Button::new(label.to_owned())
                        .style(ButtonStyle::Secondary)
                        .size(ButtonSize::Small)
                        .on_click(move || match ravenrs_report_export::export(&document, format, None) {
                            Ok(file) => session.hand_file(file.name, file.bytes, &picker),
                            Err(error) => {
                                session.show_message(Message::new(error.to_string(), MessageKind::Error));
                            }
                        }),
                )
            };
            let closing = closing.clone();
            vec![
                export("PDF", ExportFormat::Pdf),
                export("XLSX", ExportFormat::Xlsx),
                Box::new(
                    Button::new(close.clone())
                        .style(ButtonStyle::Ghost)
                        .size(ButtonSize::Small)
                        .on_click(move || closing.close_full()),
                ) as AnyView,
            ]
        });
        let panel = (!self.report.parameters.is_empty()).then(|| {
            let today = Date::from_days(ravenrs_app::local_now().days);
            ParameterPanel::new(self.report.parameters.clone(), self.applied.clone(), today)
        });
        let content = VStack::new((panel, viewer.height(VIEWER_HEIGHT)))
            .spacing(8.0)
            .align(ravenrs_layout::Align::Stretch)
            .grow(1.0);
        cx.child(&content)
    }
}

struct StoredReports {
    words: ReportWords,
}

impl ViewController for StoredReports {
    fn id(&self) -> &str {
        "StoredReports"
    }

    fn actions(&self) -> Vec<Action> {
        let (show_words, design_words) = (self.words.clone(), self.words.clone());
        vec![
            Action::simple(SHOW, self.words.show.clone())
                .category(ActionCategory::View)
                .target_entity(REPORT_DEFINITION)
                .selection(Selection::RequireSingle)
                .icon("eye")
                .on_execute(move |cx| {
                    let session = Session::of(cx).ok_or("this action needs the screens")?;
                    let record = cx.selected.first().ok_or("this action needs a report")?;
                    let (report, entity) = report_of(&session, record, &show_words)?;
                    let objects = every_record(&session, &entity.name, None);
                    show_report(&session, report, &entity, &objects, &show_words);
                    Ok(())
                }),
            Action::simple(DESIGN, self.words.design.clone())
                .category(ActionCategory::Edit)
                .target_entity(REPORT_DEFINITION)
                .selection(Selection::RequireSingle)
                .requires(Operation::Write)
                .icon("pencil")
                .on_execute(move |cx| {
                    let session = Session::of(cx).ok_or("this action needs the screens")?;
                    let record = cx.selected.first().ok_or("this action needs a report")?;
                    design_report(&session, record, &design_words)
                }),
        ]
    }
}

struct ShowInReport {
    words: ReportWords,
}

impl ViewController for ShowInReport {
    fn id(&self) -> &str {
        "ShowInReport"
    }

    fn scope(&self) -> TargetScope {
        TargetScope::any().view_kind(ViewKind::List)
    }

    fn activated(&mut self, frame: &ravenrs_app::Frame) {
        // A report of reports, or of panels, says nothing.
        let own = [REPORT_DEFINITION, crate::ANALYTICS_DASHBOARD, crate::ANALYTICS_TILE];
        let shows =
            frame.view().and_then(|view| view.entity).is_none_or(|name| !own.contains(&name.as_str()));
        if let Some(action) = frame.action(SHOW_IN_REPORT) {
            action.set_active("own records", shows);
        }
    }

    fn actions(&self) -> Vec<Action> {
        let words = self.words.clone();
        vec![
            Action::simple(SHOW_IN_REPORT, self.words.show_in_report.clone())
                .category(ActionCategory::Export)
                .icon("file-text")
                .on_execute(move |cx| {
                    let session = Session::of(cx).ok_or("this action needs the screens")?;
                    let view = cx.view.as_ref().ok_or("this action needs a list")?;
                    let entity = view
                        .entity
                        .as_deref()
                        .and_then(|name| session.entity(name))
                        .ok_or("this action needs a list")?;
                    // The ones selected, or else what the list shows.
                    let objects =
                        if cx.selected.is_empty() { session.list_records(view) } else { cx.selected.clone() };
                    let reports: Vec<Object> = every_record(&session, REPORT_DEFINITION, None)
                        .into_iter()
                        .filter(|report| report.get("target_entity") == Value::Text(entity.name.clone()))
                        .collect();
                    match reports.as_slice() {
                        [] => session
                            .show_message(Message::new(words.none_for_list.clone(), MessageKind::Warning)),
                        [one] => {
                            let (report, _) = report_of(&session, one, &words)?;
                            show_report(&session, report, &entity, &objects, &words);
                        }
                        many => {
                            let (session_for_list, words_for_list, many) =
                                (session.clone(), words.clone(), many.to_vec());
                            session.present(words.pick_report.clone(), move || {
                                let buttons: Vec<AnyView> = many
                                    .iter()
                                    .map(|record| {
                                        let name = match record.get("name") {
                                            Value::Text(name) => name,
                                            _ => String::new(),
                                        };
                                        let (session, words, record, entity, objects) = (
                                            session_for_list.clone(),
                                            words_for_list.clone(),
                                            record.clone(),
                                            entity.clone(),
                                            objects.clone(),
                                        );
                                        Box::new(Button::new(name).on_click(move || {
                                            session.close_presented();
                                            match report_of(&session, &record, &words) {
                                                Ok((report, _)) => {
                                                    show_report(&session, report, &entity, &objects, &words);
                                                }
                                                Err(why) => {
                                                    session
                                                        .show_message(Message::new(why, MessageKind::Error));
                                                }
                                            }
                                        })) as AnyView
                                    })
                                    .collect();
                                Box::new(VStack::new(buttons).spacing(8.0)) as AnyView
                            });
                        }
                    }
                    Ok(())
                }),
        ]
    }
}

/// Reports people create, keep and run: the records, showing and designing them, and showing a list
/// in one. Storage has to know [`report_entity`].
#[derive(Debug, Default)]
pub struct StoredReportsModule {
    words: ReportWords,
    predefined: Vec<(String, String, String)>,
}

impl StoredReportsModule {
    /// With these words.
    pub fn new(words: ReportWords) -> Self {
        Self { words, predefined: Vec::new() }
    }

    /// A report that comes with the application, on this entity, added once if it is not there.
    pub fn predefined(mut self, name: impl Into<String>, entity: impl Into<String>, report: Report) -> Self {
        self.predefined.push((name.into(), entity.into(), layout_to_keep(report)));
        self
    }
}

impl Module for StoredReportsModule {
    fn name(&self) -> &str {
        "StoredReports"
    }

    fn entities(&self) -> Vec<EntityInfo> {
        vec![report_entity()]
    }

    fn view_controllers(&self) -> Vec<Box<dyn ViewController>> {
        vec![
            Box::new(StoredReports { words: self.words.clone() }),
            Box::new(ShowInReport { words: self.words.clone() }),
        ]
    }

    fn initial_records(&self, space: &mut dyn ObjectSpace) -> Result<(), String> {
        let wrong = |error: ravenrs_app::SpaceError| error.to_string();
        let existing = space.query(REPORT_DEFINITION, &Query::all()).map_err(wrong)?;
        for (name, entity, layout) in &self.predefined {
            if existing.iter().any(|record| record.get("name") == Value::Text(name.clone())) {
                continue;
            }
            let mut record = space.new_object(REPORT_DEFINITION).map_err(wrong)?;
            record.set("name", name.clone());
            record.set("target_entity", entity.clone());
            record.set("layout", layout.clone());
            space.save(record).map_err(wrong)?;
        }
        Ok(())
    }
}

/// Says something wrong with a report on the screen.
pub(crate) fn problem(title: &str, why: String) -> AnyView {
    Box::new(Alert::new(title.to_owned(), why).tone(Tone::Danger))
}
