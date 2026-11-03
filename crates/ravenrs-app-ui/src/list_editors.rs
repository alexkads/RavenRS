//! Other ways to show a list than a grid: a chart, a month, a timeline of resources and a tree.
//!
//! A view picks one by `EditorType` in the model, or a way to see the list by its own. What each
//! one needs comes from a node under the view, `ChartSettings`, `CalendarSettings`,
//! `SchedulerSettings` or `TreeSettings`, as text a person can change like any other setting, and
//! what is not said follows from the kinds of the fields.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use ravenrs_app::{
    CONTEXT_SAVE, EntityInfo, FieldInfo, FieldKind, Object, Operation, Query, Value, Violation,
};
use ravenrs_criteria::DateTime;
use ravenrs_reactive::{Binding, State};
use ravenrs_widgets::{
    Alert, AnyView, BuildCx, CalendarEvent, Chart, ChartKind, EventCalendar, NodeId, Resource, Scheduler,
    SchedulerChange, SchedulerEvent, TimeAxis, TimePoint, Tone, TreeNode, TreeView, View,
};

use crate::ListEditorRequest;
use crate::Session;
use crate::values::{date_of, display};

/// The name of the editor that shows a list as a chart.
pub const CHART_EDITOR: &str = "Chart";
/// The name of the editor that shows a list on the days of a month.
pub const CALENDAR_EDITOR: &str = "Calendar";
/// The name of the editor that shows a list on a timeline, a row per resource.
pub const SCHEDULER_EDITOR: &str = "Scheduler";
/// The name of the editor that shows a list as a tree.
pub const TREE_EDITOR: &str = "Tree";

/// How long a record with no end shows, in minutes.
const NO_END_MINUTES: i64 = 60;
const CHART_HEIGHT: f32 = 420.0;
const TREE_HEIGHT: f32 = 520.0;
const TONES: [Tone; 5] = [Tone::Accent, Tone::Success, Tone::Warning, Tone::Danger, Tone::Neutral];

/// Puts them where a view can pick them, by name.
pub(crate) fn register(session: &Session) {
    session.register_list_editor(CHART_EDITOR, |request| Box::new(Shown::new(request, chart)) as AnyView);
    session
        .register_list_editor(CALENDAR_EDITOR, |request| Box::new(Shown::new(request, calendar)) as AnyView);
    session.register_list_editor(SCHEDULER_EDITOR, |request| {
        Box::new(Shown::new(request, scheduler)) as AnyView
    });
    session.register_list_editor(TREE_EDITOR, |request| Box::new(Shown::new(request, tree)) as AnyView);
}

type Build = fn(&Data, &mut BuildCx<'_>) -> Result<NodeId, String>;

/// What every one of them works from.
struct Data {
    session: Session,
    view: String,
    entity: EntityInfo,
    columns: Vec<(FieldInfo, String)>,
    objects: Rc<Vec<Object>>,
    on_open: crate::OpenHandler,
    on_select: Rc<dyn Fn(Vec<Object>)>,
}

/// A view that says what is missing instead of showing something wrong.
struct Shown {
    data: Data,
    build: Build,
}

impl Shown {
    fn new(request: &ListEditorRequest, build: Build) -> Self {
        Self {
            data: Data {
                session: request.session.clone(),
                view: request.view.id.clone(),
                entity: request.entity.clone(),
                columns: request.columns.clone(),
                objects: request.objects.clone(),
                on_open: request.on_open.clone(),
                on_select: request.on_select.clone(),
            },
            build,
        }
    }
}

impl View for Shown {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        match (self.build)(&self.data, cx) {
            Ok(node) => node,
            Err(why) => {
                let title = self.data.entity.caption.clone();
                cx.child(&Alert::new(title, why).tone(Tone::Warning))
            }
        }
    }
}

impl Data {
    fn setting(&self, node: &str, name: &str) -> Option<String> {
        let model = self.session.model();
        model.at(&["Views", &self.view, node]).and_then(|settings| settings.text(name)).map(str::to_owned)
    }

    /// The field a setting names, or, if it says nothing, the first that fits.
    fn field(
        &self,
        node: &str,
        name: &str,
        fits: impl Fn(&FieldInfo) -> bool,
    ) -> Result<Option<&FieldInfo>, String> {
        match self.setting(node, name) {
            Some(named) => match self.entity.field_named(&named) {
                Some(field) => Ok(Some(field)),
                None => Err(self.words(&format!("{node}/{name}: {named}"))),
            },
            None => Ok(self.columns.iter().map(|(field, _)| field).find(|field| fits(field))),
        }
    }

    fn words(&self, what: &str) -> String {
        format!("{} {what}", self.session.words().list_editor_needs)
    }

    fn key_of(&self, object: &Object) -> Value {
        self.entity.key_field().map_or(Value::Null, |key| object.get(&key.name))
    }

    /// What names the record, for whoever shows one.
    fn title_of(&self, node: &str, object: &Object) -> String {
        let field = self
            .setting(node, "Title")
            .and_then(|name| self.entity.field_named(&name))
            .or_else(|| self.entity.default_property());
        field.map_or_else(
            || ravenrs_app::key_text(&self.key_of(object)).unwrap_or_default(),
            |field| display(field, &object.get(&field.name)),
        )
    }

    /// What names each record of the entity a reference points to.
    fn names_of(&self, target: &str) -> BTreeMap<String, String> {
        let Some(entity) = self.session.entity(target) else { return BTreeMap::new() };
        let (Some(key), Some(name)) = (entity.key_field(), entity.default_property()) else {
            return BTreeMap::new();
        };
        let found = self.session.frame().space().borrow().query(target, &Query::all());
        found
            .unwrap_or_default()
            .iter()
            .map(|object| (display(key, &object.get(&key.name)), display(name, &object.get(&name.name))))
            .collect()
    }

    fn tone_of(&self, node: &str, object: &Object) -> Tone {
        let field = self.setting(node, "Tone").and_then(|name| self.entity.field_named(&name).cloned());
        let Some(field) = field else { return Tone::Accent };
        let FieldKind::Enum(options) = &field.kind else { return Tone::Accent };
        let value = object.get(&field.name);
        options
            .iter()
            .position(|option| value == Value::Text(option.code.clone()))
            .map_or(Tone::Neutral, |at| TONES[at % TONES.len()])
    }

    fn is_done(&self, node: &str, object: &Object) -> bool {
        self.setting(node, "Done").is_some_and(|name| object.get(&name) == Value::Bool(true))
    }

    fn can(&self, operation: Operation, record: Option<&Object>) -> bool {
        self.session.frame().security().is_none_or(|security| {
            security.with(|permissions| permissions.can(operation, &self.entity.name, record))
        })
    }

    /// Writes a record changed on the screen: its rules first, then in a commit of its own.
    fn store(&self, object: Object) {
        let session = &self.session;
        let errors: Vec<String> = session
            .violations_of(&object, CONTEXT_SAVE)
            .into_iter()
            .filter(Violation::is_error)
            .map(|violation| violation.message)
            .collect();
        if !errors.is_empty() {
            session.set_error(Some(errors.join("; ")));
            session.records_changed();
            return;
        }
        let space = session.frame().space();
        let mut space = space.borrow_mut();
        let result = space.save(object).and_then(|()| space.commit());
        if let Err(error) = result {
            space.rollback();
            session.set_error(Some(error.to_string()));
        } else {
            session.set_error(None);
        }
        drop(space);
        session.records_changed();
    }
}

fn is_date(field: &FieldInfo) -> bool {
    matches!(field.kind, FieldKind::Date | FieldKind::DateTime)
}

fn minutes_of(value: &Value) -> Option<TimePoint> {
    match value {
        Value::Date(when) => Some(TimePoint::from_minutes(when.days * 1440 + i64::from(when.seconds) / 60)),
        _ => None,
    }
}

fn value_of(time: TimePoint) -> Value {
    let minutes = time.minutes();
    Value::Date(DateTime { days: minutes.div_euclid(1440), seconds: (minutes.rem_euclid(1440) * 60) as u32 })
}

/// How the values of one bar or point come together.
#[derive(Clone, Copy, PartialEq)]
enum Summary {
    Sum,
    Count,
    Average,
    Min,
    Max,
}

impl Summary {
    fn named(name: &str) -> Self {
        match name {
            "Count" => Self::Count,
            "Average" => Self::Average,
            "Min" => Self::Min,
            "Max" => Self::Max,
            _ => Self::Sum,
        }
    }

    fn of(self, values: &[f64]) -> f64 {
        match self {
            Self::Sum => values.iter().sum(),
            Self::Count => values.len() as f64,
            Self::Average if values.is_empty() => 0.0,
            Self::Average => values.iter().sum::<f64>() / values.len() as f64,
            Self::Min => values.iter().copied().reduce(f64::min).unwrap_or(0.0),
            Self::Max => values.iter().copied().reduce(f64::max).unwrap_or(0.0),
        }
    }
}

fn is_number(field: &FieldInfo) -> bool {
    matches!(field.kind, FieldKind::Integer | FieldKind::Decimal(_) | FieldKind::Money)
}

/// A date as what it is grouped by. The text sorts as time goes.
fn date_group(value: &Value, interval: &str) -> Option<String> {
    let date = date_of(value)?;
    let (year, month, _) = (date.year(), date.month(), date.day());
    Some(match interval {
        "Year" => format!("{year:04}"),
        "Month" => format!("{year:04}-{month:02}"),
        _ => date.to_string(),
    })
}

fn chart(data: &Data, cx: &mut BuildCx<'_>) -> Result<NodeId, String> {
    const NODE: &str = "ChartSettings";
    let argument = data
        .field(NODE, "Argument", |field| !is_number(field))?
        .ok_or_else(|| data.words("ChartSettings/Argument"))?;
    let captions: BTreeMap<&str, &str> =
        data.columns.iter().map(|(field, caption)| (field.name.as_str(), caption.as_str())).collect();
    // `cost:Sum,hours:Average`, or every number shown, added up, or how many there are.
    let series: Vec<(Option<&FieldInfo>, Summary, String)> = match data.setting(NODE, "Series") {
        Some(text) => text
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(|part| {
                let (name, summary) = part.split_once(':').unwrap_or((part, "Sum"));
                let field = data
                    .entity
                    .field_named(name)
                    .ok_or_else(|| data.words(&format!("{NODE}/Series: {name}")))?;
                let caption = captions.get(name).map_or(field.caption.as_str(), |caption| caption);
                Ok((Some(field), Summary::named(summary), caption.to_owned()))
            })
            .collect::<Result<_, String>>()?,
        None => data
            .columns
            .iter()
            .filter(|(field, _)| is_number(field))
            .map(|(field, caption)| (Some(field), Summary::Sum, caption.clone()))
            .collect(),
    };
    let series = if series.is_empty() {
        vec![(None, Summary::Count, data.session.words().chart_count.clone())]
    } else {
        series
    };
    let names = match &argument.kind {
        FieldKind::Reference(target) => data.names_of(target),
        _ => BTreeMap::new(),
    };
    let interval = data.setting(NODE, "Interval").unwrap_or_else(|| "Day".to_owned());
    let label_of = |object: &Object| {
        let value = object.get(&argument.name);
        if is_date(argument) {
            return date_group(&value, &interval).unwrap_or_default();
        }
        let shown = display(argument, &value);
        names.get(&shown).cloned().unwrap_or(shown)
    };
    // In the order they come, except for dates, which go as time does.
    let mut order: Vec<String> = Vec::new();
    let mut groups: BTreeMap<String, Vec<&Object>> = BTreeMap::new();
    for object in data.objects.iter() {
        let label = label_of(object);
        if !groups.contains_key(&label) {
            order.push(label.clone());
        }
        groups.entry(label).or_default().push(object);
    }
    if is_date(argument) {
        order.sort();
    }
    let money =
        series.iter().all(|(field, ..)| field.is_some_and(|field| matches!(field.kind, FieldKind::Money)));
    let mut view = Chart::new(kind_named(data.setting(NODE, "Kind").as_deref()))
        .categories(order.clone())
        .height(CHART_HEIGHT)
        .show_value_axis()
        .alt(data.entity.caption.clone());
    for (field, summary, caption) in &series {
        let values = order.iter().map(|label| {
            let rows = &groups[label];
            let numbers: Vec<f64> = match field {
                Some(field) => rows
                    .iter()
                    .filter_map(|object| match object.get(&field.name) {
                        Value::Number(number) => Some(number),
                        _ => None,
                    })
                    .collect(),
                None => vec![1.0; rows.len()],
            };
            let value = summary.of(&numbers);
            // Money is kept in cents and read in whole units.
            let in_cents = field.is_some_and(|field| matches!(field.kind, FieldKind::Money));
            if in_cents && *summary != Summary::Count { value / 100.0 } else { value }
        });
        view = view.series(caption.clone(), values.collect::<Vec<_>>());
    }
    if money {
        let words = data.session.words();
        let (prefix, thousands, decimal) = (words.money_prefix.clone(), words.thousands, words.decimal);
        view = view.format(move |value| {
            ravenrs_widgets::grid::NumberFormat {
                prefix: prefix.clone(),
                decimals: 2,
                thousands,
                decimal,
                ..ravenrs_widgets::grid::NumberFormat::default()
            }
            .format(value)
        });
    }
    Ok(cx.child(&view))
}

fn kind_named(name: Option<&str>) -> ChartKind {
    match name {
        Some("Line") => ChartKind::Line,
        Some("Area") => ChartKind::Area,
        Some("Pie") => ChartKind::Pie,
        Some("Doughnut") => ChartKind::Doughnut,
        _ => ChartKind::Bar,
    }
}

fn calendar(data: &Data, cx: &mut BuildCx<'_>) -> Result<NodeId, String> {
    const NODE: &str = "CalendarSettings";
    let start =
        data.field(NODE, "Start", is_date)?.ok_or_else(|| data.words("CalendarSettings/Start"))?.name.clone();
    let today = data.session.today();
    let month: State<_> = cx.use_state(|| today.first_of_month());
    let day: State<Option<_>> = cx.use_state(|| None);
    let events = data.objects.iter().enumerate().filter_map(|(at, object)| {
        let date = date_of(&object.get(&start))?;
        Some(
            CalendarEvent::new(at as u64, date, data.title_of(NODE, object))
                .tone(data.tone_of(NODE, object))
                .done(data.is_done(NODE, object)),
        )
    });
    let (objects, on_open, key) =
        (data.objects.clone(), data.on_open.clone(), data.entity.key_field().cloned());
    let view = EventCalendar::new(month.binding(), day.binding())
        .today(today)
        .events(events.collect::<Vec<_>>())
        .on_event(move |at| {
            if let (Some(object), Some(key)) = (objects.get(at as usize), &key) {
                on_open(&object.get(&key.name));
            }
        });
    Ok(cx.child(&view))
}

fn scheduler(data: &Data, cx: &mut BuildCx<'_>) -> Result<NodeId, String> {
    const NODE: &str = "SchedulerSettings";
    let start = data
        .field(NODE, "Start", is_date)?
        .ok_or_else(|| data.words("SchedulerSettings/Start"))?
        .name
        .clone();
    let end = data
        .field(NODE, "End", |field| is_date(field) && field.name != start)?
        .map(|field| field.name.clone());
    let resource =
        data.field(NODE, "Resource", |field| matches!(field.kind, FieldKind::Reference(_)))?.cloned();
    // A row for each record the reference can point to, and one for what points nowhere.
    let names: Vec<(String, String)> = match resource.as_ref().map(|field| &field.kind) {
        Some(FieldKind::Reference(target)) => data.names_of(target).into_iter().collect(),
        _ => Vec::new(),
    };
    let row_of = |object: &Object| {
        let resource = resource.as_ref()?;
        let key = display(resource, &object.get(&resource.name));
        names.iter().position(|(known, _)| *known == key).map(|at| at as u64 + 1)
    };
    let mut rows: Vec<Resource> =
        names.iter().enumerate().map(|(at, (_, name))| Resource::new(at as u64 + 1, name.clone())).collect();
    if resource.is_none() || data.objects.iter().any(|object| row_of(object).is_none()) {
        let caption = if resource.is_none() {
            data.entity.caption.clone()
        } else {
            data.session.words().no_resource.clone()
        };
        rows.insert(0, Resource::new(0, caption));
    }
    let writes = data.can(Operation::Write, None);
    let events: Vec<SchedulerEvent> = data
        .objects
        .iter()
        .enumerate()
        .filter_map(|(at, object)| {
            let from = minutes_of(&object.get(&start))?;
            let to = end
                .as_ref()
                .and_then(|end| minutes_of(&object.get(end)))
                .filter(|to| *to > from)
                .unwrap_or_else(|| from.plus_minutes(NO_END_MINUTES));
            Some(
                SchedulerEvent::new(
                    at as u64,
                    row_of(object).unwrap_or(0),
                    from,
                    to,
                    data.title_of(NODE, object),
                )
                .tone(data.tone_of(NODE, object))
                .done(data.is_done(NODE, object))
                .locked(!data.can(Operation::Write, Some(object))),
            )
        })
        .collect();
    let today = data.session.today();
    let axis: State<TimeAxis> = cx.use_state(|| TimeAxis::days(today.first_of_month(), 31));
    let selected: State<Option<u64>> = cx.use_state(|| None);
    let (read, write) = (selected.clone(), selected.clone());
    let (objects, on_select) = (data.objects.clone(), data.on_select.clone());
    let selection = Binding::new(
        move || read.get(),
        move |id: Option<u64>| {
            write.set(id);
            on_select(id.and_then(|id| objects.get(id as usize).cloned()).into_iter().collect());
        },
    );
    let shared = Rc::new(Keep {
        data: Data {
            session: data.session.clone(),
            view: data.view.clone(),
            entity: data.entity.clone(),
            columns: Vec::new(),
            objects: data.objects.clone(),
            on_open: data.on_open.clone(),
            on_select: data.on_select.clone(),
        },
        start,
        end,
        resource,
        keys: names.into_iter().map(|(key, _)| key).collect(),
    });
    let (changing, opening) = (shared.clone(), shared.clone());
    let mut view = Scheduler::new(axis.binding())
        .resources(rows)
        .events(events)
        .selection(selection)
        .read_only(!writes)
        .on_change(move |change| changing.moved(change))
        .on_open(move |at| opening.opened(at));
    if data.can(Operation::Create, None) {
        let creating = shared;
        view = view.on_create(move |change| creating.created(change));
    }
    Ok(cx.child(&view))
}

/// What the timeline needs once something happens on it.
struct Keep {
    data: Data,
    start: String,
    end: Option<String>,
    resource: Option<FieldInfo>,
    keys: Vec<String>,
}

impl Keep {
    /// The value of the reference for a row, as the kind of the key it points to.
    fn resource_value(&self, row: u64) -> Value {
        let Some(key) = row.checked_sub(1).and_then(|at| self.keys.get(at as usize)) else {
            return Value::Null;
        };
        let Some(FieldKind::Reference(target)) = self.resource.as_ref().map(|field| &field.kind) else {
            return Value::Null;
        };
        let numbered = self
            .data
            .session
            .entity(target)
            .and_then(|entity| entity.key_field().map(|key| matches!(key.kind, FieldKind::Integer)));
        match (numbered, key.parse::<f64>()) {
            (Some(true), Ok(number)) => Value::Number(number),
            _ => Value::Text(key.clone()),
        }
    }

    fn moved(&self, change: SchedulerChange) {
        let Some(mut object) = change.id.and_then(|id| self.data.objects.get(id as usize).cloned()) else {
            return;
        };
        object.set(&self.start, value_of(change.start));
        if let Some(end) = &self.end {
            object.set(end, value_of(change.end));
        }
        if let Some(resource) = &self.resource {
            object.set(&resource.name, self.resource_value(change.resource));
        }
        self.data.store(object);
    }

    fn opened(&self, at: u64) {
        if let Some(object) = self.data.objects.get(at as usize) {
            (self.data.on_open)(&self.data.key_of(object));
        }
    }

    fn created(&self, change: SchedulerChange) {
        let session = &self.data.session;
        session.open_new(&self.data.entity.name);
        session.set_form_value(&self.start, value_of(change.start));
        if let Some(end) = &self.end {
            session.set_form_value(end, value_of(change.end));
        }
        if let Some(resource) = &self.resource {
            session.set_form_value(&resource.name, self.resource_value(change.resource));
        }
    }
}

fn tree(data: &Data, cx: &mut BuildCx<'_>) -> Result<NodeId, String> {
    const NODE: &str = "TreeSettings";
    let entity_name = data.entity.name.clone();
    let parent =
        data.setting(NODE, "Parent")
            .and_then(|name| data.entity.field_named(&name))
            .or_else(|| {
                data.entity.fields.iter().find(
                    |field| matches!(&field.kind, FieldKind::Reference(target) if *target == entity_name),
                )
            })
            .ok_or_else(|| data.words("TreeSettings/Parent"))?;
    let Some(key_field) = data.entity.key_field() else {
        return Err(data.words("TreeSettings/Key"));
    };
    let key_of = |object: &Object| display(key_field, &object.get(&key_field.name));
    let parent_of = |object: &Object| display(parent, &object.get(&parent.name));
    let known: BTreeSet<String> = data.objects.iter().map(key_of).collect();
    let mut children: BTreeMap<String, Vec<&Object>> = BTreeMap::new();
    let mut roots: Vec<&Object> = Vec::new();
    for object in data.objects.iter() {
        let above = parent_of(object);
        // One whose parent cannot be seen starts its own branch.
        if above.is_empty() || !known.contains(&above) || above == key_of(object) {
            roots.push(object);
        } else {
            children.entry(above).or_default().push(object);
        }
    }
    fn node_of(
        data: &Data,
        object: &Object,
        children: &BTreeMap<String, Vec<&Object>>,
        key_of: &dyn Fn(&Object) -> String,
        seen: &mut BTreeSet<String>,
    ) -> TreeNode {
        let key = key_of(object);
        let title = data.title_of("TreeSettings", object);
        seen.insert(key.clone());
        let mut node = TreeNode::new(key.clone(), title);
        for child in children.get(&key).map_or(&[][..], Vec::as_slice) {
            if !seen.contains(&key_of(child)) {
                node = node.child(node_of(data, child, children, key_of, seen));
            }
        }
        node
    }
    let mut seen = BTreeSet::new();
    let mut nodes: Vec<TreeNode> =
        roots.iter().map(|object| node_of(data, object, &children, &key_of, &mut seen)).collect();
    // What only points at itself in a circle has no root, and still has to show.
    for object in data.objects.iter() {
        if !seen.contains(&key_of(object)) {
            nodes.push(node_of(data, object, &children, &key_of, &mut seen));
        }
    }
    let parents: BTreeSet<String> = children.keys().cloned().collect();
    let expanded: State<BTreeSet<String>> = cx.use_state(|| parents);
    let selected: State<Option<String>> = cx.use_state(|| None);
    let by_key: BTreeMap<String, Object> =
        data.objects.iter().map(|object| (key_of(object), object.clone())).collect();
    let by_key = Rc::new(by_key);
    let (read, write, choosing, on_select) =
        (selected.clone(), selected.clone(), by_key.clone(), data.on_select.clone());
    let selection = Binding::new(
        move || read.get(),
        move |key: Option<String>| {
            on_select(key.as_ref().and_then(|key| choosing.get(key).cloned()).into_iter().collect());
            write.set(key);
        },
    );
    let (on_open, key_name) = (data.on_open.clone(), key_field.name.clone());
    let view = TreeView::new(nodes)
        .selected(selection)
        .expanded(expanded.binding())
        .height(TREE_HEIGHT)
        .label(data.entity.caption.clone())
        .on_open(move |key| {
            if let Some(object) = by_key.get(key) {
                on_open(&object.get(&key_name));
            }
        });
    Ok(cx.child(&view))
}
