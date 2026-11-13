//! Many records: a [`DataGrid`] over the object space, shaped by the model.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use ravenrs_app::{
    AppearanceTarget, CONTEXT_SAVE, Criteria, DataAccess, EntityInfo, FieldInfo, FieldKind, Frame, Message,
    MessageKind, ModelNode, Object, Query, Severity, Value, ViewInfo, validate_in_place,
};
use ravenrs_grid_export::export;
use ravenrs_reactive::{Binding, State};
use ravenrs_widgets::grid::{GridQuery, Row, Value as Cell, to_table};
use ravenrs_widgets::{
    AnyView, BuildCx, CellEdit, DataGrid, GridCellStyle, GridColumn, GridCurrent, GridData, GridPage,
    GridRequest, GridResponder, GridRowAction, GridTable, Keyed, NodeId, RemoteRows, SelectionMode,
    SortDirection, Text, View,
};

use crate::session::Session;
use crate::values::{
    cell_of, column_of, display, editable_in_list, key_from_text, key_text, semantic_of, value_of_cell,
};

/// What opening a row of a list does, with its key.
pub type OpenHandler = Rc<dyn Fn(&Value)>;

/// What a list editor is asked to show: the records as the list reads them, and how to act on them.
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ListEditorRequest {
    /// The application it lives in.
    pub session: Session,
    /// The view it shows.
    pub view: ViewInfo,
    /// The entity of the records.
    pub entity: EntityInfo,
    /// The fields the list shows, with what their columns say.
    pub columns: Vec<(FieldInfo, String)>,
    /// The records, in the list's order.
    pub objects: Rc<Vec<Object>>,
    /// Opens a record, by its key.
    pub on_open: OpenHandler,
    /// Says which records are selected, which turns actions on and off.
    pub on_select: Rc<dyn Fn(Vec<Object>)>,
}

impl std::fmt::Debug for ListEditorRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ListEditorRequest").field("view", &self.view).finish_non_exhaustive()
    }
}

/// Shows a list some other way than in a grid.
pub(crate) type ListEditorFactory = Rc<dyn Fn(&ListEditorRequest) -> AnyView>;

/// What gives a list its records when no storage keeps them.
pub type ListSource = Rc<dyn Fn() -> Vec<Object>>;

/// What a column ends up as, from its field and the one the field gives.
pub type ColumnShape = Rc<dyn Fn(&FieldInfo, GridColumn) -> GridColumn>;

/// What a grid ends up as, from the one the list made.
pub type GridShape = Rc<dyn Fn(DataGrid) -> DataGrid>;

/// A list read from storage, kept until something may have changed it.
#[derive(Clone)]
struct Loaded {
    revision: u64,
    asked: String,
    data: GridData,
    objects: Rc<BTreeMap<String, Object>>,
    ordered: Rc<Vec<Object>>,
    /// On a list people edit: the cells that fail a rule, by the key of the record and the field.
    invalid: Rc<BTreeSet<(String, String)>>,
}

/// A list that brings its rows as they are needed.
#[derive(Clone)]
struct Remote {
    asked: String,
    revision: u64,
    rows: RemoteRows,
}

/// A field a list shows, what its column says, and whether the column shows.
type Shown<'a> = (&'a FieldInfo, String, bool);

/// The fields a list shows, in the order to show them, with the key last and out of sight, so
/// a row can always be found.
fn shown_fields<'a>(entity: &'a EntityInfo, view: &'a ModelNode, only: Option<&[String]>) -> Vec<Shown<'a>> {
    let all = view.get("Columns");
    let caption = |id: &str, field: &FieldInfo| {
        all.and_then(|columns| columns.get(id))
            .and_then(|column| column.text("Caption"))
            .unwrap_or(&field.caption)
            .to_owned()
    };
    let chosen: Vec<&str> = match only {
        Some(names) => names.iter().map(String::as_str).collect(),
        None => all
            .map(|columns| columns.in_order())
            .unwrap_or_default()
            .into_iter()
            .map(|column| column.id.as_str())
            .collect(),
    };
    let mut fields: Vec<Shown<'a>> = chosen
        .into_iter()
        .filter_map(|id| entity.field_named(id).map(|field| (field, caption(id, field), true)))
        .collect();
    if let Some(key) = entity.key_field()
        && !fields.iter().any(|(field, ..)| field.is_key)
    {
        fields.push((key, key.caption.clone(), false));
    }
    fields
}

/// The records of an entity in a grid: search, filters, sorting and export come with it. Opening
/// a row opens its form.
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ListView {
    session: Session,
    frame: Frame,
    view: ViewInfo,
    on_open: Option<OpenHandler>,
    compact: bool,
    row_actions: Vec<GridRowAction>,
    cell_text: Option<CellText>,
    filter: Option<Criteria>,
    sort: Vec<(String, bool)>,
    column_shape: Option<ColumnShape>,
    grid_shape: Option<GridShape>,
    /// What goes without saying where this list is.
    implied: Option<String>,
    single: bool,
    plain: bool,
    remote: bool,
    source: Option<ListSource>,
}

/// What a cell says instead of its value, for a field and a record; nothing means its value.
pub type CellText = Rc<dyn Fn(&str, &Object) -> Option<String>>;

impl std::fmt::Debug for ListView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ListView").field("view", &self.view).finish_non_exhaustive()
    }
}

/// How a list reads its rows, without the view around it, so a remote loader can keep one.
#[derive(Clone)]
struct Reader {
    session: Session,
    frame: Frame,
    cell_text: Option<CellText>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Reader {
    /// What names each record of an entity, by its key as text.
    fn names_of(&self, entity_name: &str) -> BTreeMap<String, String> {
        let Some(entity) = self.session.entity(entity_name) else { return BTreeMap::new() };
        let (Some(key), Some(name)) = (entity.key_field(), entity.default_property()) else {
            return BTreeMap::new();
        };
        let found = self.frame.space().borrow().query(entity_name, &Query::all());
        found
            .unwrap_or_default()
            .iter()
            .map(|object| (display(key, &object.get(&key.name)), display(name, &object.get(&name.name))))
            .collect()
    }

    /// A reference shows what names the record it points to, read once for the whole list.
    fn names(&self, fields: &[FieldInfo]) -> BTreeMap<String, BTreeMap<String, String>> {
        fields
            .iter()
            .filter_map(|field| match &field.kind {
                FieldKind::Reference(target) => Some((field.name.clone(), self.names_of(target))),
                _ => None,
            })
            .collect()
    }

    fn cell(
        &self,
        names: &BTreeMap<String, BTreeMap<String, String>>,
        field: &FieldInfo,
        object: &Object,
    ) -> Cell {
        let value = object.get(&field.name);
        if let Some(known) = names.get(&field.name)
            && !matches!(value, Value::Null)
        {
            let text = display(field, &value);
            return Cell::Text(known.get(&text).cloned().unwrap_or(text));
        }
        match self.cell_text.as_ref().and_then(|text| text(&field.name, object)) {
            Some(text) if text.is_empty() => Cell::Empty,
            Some(text) => Cell::Text(text),
            None => cell_of(field, &value),
        }
    }

    fn rows(&self, fields: &[FieldInfo], objects: &[Object]) -> Vec<Row> {
        let names = self.names(fields);
        objects
            .iter()
            .map(|object| fields.iter().map(|field| self.cell(&names, field, object)).collect())
            .collect()
    }
}

/// `text` as a literal of the expression language.
fn literal(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

fn both(first: Option<Criteria>, second: Option<Criteria>) -> Option<Criteria> {
    match (first, second) {
        (Some(first), Some(second)) => Some(first.and(second)),
        (first, second) => first.or(second),
    }
}

/// The part of a remote request that says which rows, over the fields of its columns.
fn request_filter(request: &GridRequest, fields: &[FieldInfo]) -> Option<Criteria> {
    filter_of(&request.filters, &request.search, fields)
}

/// What filters by column and a search ask of storage, over the fields of the columns.
fn filter_of(filters: &[String], search: &str, fields: &[FieldInfo]) -> Option<Criteria> {
    let searchable =
        |field: &FieldInfo| matches!(field.kind, FieldKind::Text | FieldKind::LongText | FieldKind::Enum(_));
    let contains = |field: &FieldInfo, text: &str| {
        Criteria::parse(&format!("Contains(Upper([{}]), Upper({}))", field.name, literal(text))).ok()
    };
    let by_column = filters.iter().enumerate().fold(None, |all, (at, text)| {
        let this = fields
            .get(at)
            .filter(|field| searchable(field) && !text.trim().is_empty())
            .and_then(|field| contains(field, text.trim()));
        both(all, this)
    });
    let typed = search.trim();
    let searched = if typed.is_empty() {
        None
    } else {
        fields
            .iter()
            .filter(|field| searchable(field))
            .filter_map(|field| contains(field, typed))
            .reduce(Criteria::or)
    };
    both(by_column, searched)
}

/// What a list on screen shows now, as a table, for a report to take the same rows.
pub(crate) type ShownList = Rc<dyn Fn() -> Option<GridTable>>;

/// Every record of a remote list that its filters and search let through, in its order: the grid
/// holds only the pages it has brought.
fn whole_remote(
    reader: &Reader,
    entity: &EntityInfo,
    fields: &[FieldInfo],
    base: Option<&Criteria>,
    sort: &[(String, bool)],
    asked: &GridQuery,
) -> Result<Vec<Row>, String> {
    let mut query = Query::all();
    if let Some(filter) = both(base.cloned(), filter_of(&asked.filters, &asked.search, fields)) {
        query = query.filter(filter);
    }
    let asked_sort: Vec<(String, bool)> = asked
        .sort
        .iter()
        .filter_map(|(column, direction)| {
            let field = fields.get(*column)?;
            Some((field.name.clone(), matches!(direction, SortDirection::Ascending)))
        })
        .collect();
    let order = if asked_sort.is_empty() { sort.to_vec() } else { asked_sort };
    for (field, ascending) in order {
        query = query.sort(field, ascending);
    }
    if let Some(key) = entity.key_field() {
        query = query.sort(key.name.clone(), true);
    }
    let objects = reader.frame.space().borrow().query(&entity.name, &query).map_err(|e| e.to_string())?;
    Ok(reader.rows(fields, &objects))
}

/// Rows brought from storage a chunk at a time. With no sorting asked for, a chunk continues from
/// the key of the row before it, which does not get slower the further down it goes.
fn answer_remote(
    reader: &Reader,
    entity: &EntityInfo,
    fields: &[FieldInfo],
    base: Option<&Criteria>,
    sort: &[(String, bool)],
    request: &GridRequest,
    answer: GridResponder,
) {
    let filter = both(base.cloned(), request_filter(request, fields));
    let key = entity.key_field();
    let asked_sort: Vec<(String, bool)> = request
        .sort
        .iter()
        .filter_map(|(column, direction)| {
            let field = fields.get(*column)?;
            Some((field.name.clone(), matches!(direction, SortDirection::Ascending)))
        })
        .collect();
    let keyset = asked_sort.is_empty() && sort.is_empty();
    let after = keyset
        .then(|| {
            let at = fields.iter().position(|field| field.is_key)?;
            let (key, cell) = (key?, request.after.as_ref()?.get(at)?);
            let after = crate::editors::literal_of(&key_from_text(entity, &cell.key_text()));
            Criteria::parse(&format!("[{}] > {after}", key.name)).ok()
        })
        .flatten();
    let offset = if after.is_some() { 0 } else { request.offset };
    let mut query = Query::all().page(offset, request.limit + 1);
    if let Some(filter) = both(filter, after) {
        query = query.filter(filter);
    }
    let order = if asked_sort.is_empty() { sort.to_vec() } else { asked_sort };
    for (field, ascending) in order {
        query = query.sort(field, ascending);
    }
    if keyset && let Some(key) = key {
        query = query.sort(key.name.clone(), true);
    }
    let found = reader.frame.space().borrow().query(&entity.name, &query);
    match found {
        Ok(mut objects) => {
            let has_more = objects.len() > request.limit;
            objects.truncate(request.limit);
            answer.send(GridPage::new(reader.rows(fields, &objects), has_more));
        }
        Err(error) => answer.fail(error.to_string()),
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ListView {
    /// The list of this view in the model, reading through this frame.
    pub fn new(session: Session, frame: Frame, view: ViewInfo) -> Self {
        Self {
            session,
            frame,
            view,
            on_open: None,
            compact: false,
            row_actions: Vec::new(),
            cell_text: None,
            filter: None,
            sort: Vec::new(),
            column_shape: None,
            grid_shape: None,
            implied: None,
            single: false,
            plain: false,
            remote: false,
            source: None,
        }
    }

    /// Only the rows and their headers, as tall as they need, for a few records beside a form.
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }

    /// A button at the end of every row.
    pub fn row_action(mut self, action: GridRowAction) -> Self {
        self.row_actions.push(action);
        self
    }

    /// Says what a cell shows, when its value alone is not it.
    pub fn cell_text(mut self, text: CellText) -> Self {
        self.cell_text = Some(text);
        self
    }

    /// What opening a row does, with its key. Its form by default.
    pub fn on_open(mut self, handler: OpenHandler) -> Self {
        self.on_open = Some(handler);
        self
    }

    /// Shows only the records that meet this, besides what the way picked to see the list asks.
    pub fn filter(mut self, filter: Option<Criteria>) -> Self {
        self.filter = filter;
        self
    }

    /// Orders the records by this field, after what was already asked for.
    pub fn sort(mut self, field: impl Into<String>, ascending: bool) -> Self {
        self.sort.push((field.into(), ascending));
        self
    }

    /// Changes the column of a field: a template, a width, a summary, or one of its own.
    pub fn column_shape(mut self, shape: impl Fn(&FieldInfo, GridColumn) -> GridColumn + 'static) -> Self {
        self.column_shape = Some(Rc::new(shape));
        self
    }

    /// Leaves out the column of this field, whose value is the same on every row.
    pub fn implied(mut self, field: impl Into<String>) -> Self {
        self.implied = Some(field.into());
        self
    }

    /// Changes the grid before it goes on the screen, for what the list has no word for.
    pub fn grid_shape(mut self, shape: impl Fn(DataGrid) -> DataGrid + 'static) -> Self {
        self.grid_shape = Some(Rc::new(shape));
        self
    }

    /// Only one row selected at a time.
    pub fn single_selection(mut self) -> Self {
        self.single = true;
        self
    }

    /// Leaves out the filter builder and exporting to files, for a screen that has its own.
    pub fn plain_toolbar(mut self) -> Self {
        self.plain = true;
        self
    }

    /// Brings rows from storage as they are needed, for lists too long to read at once.
    /// Grouping needs the rows at hand, so it is off, and searching covers only fields with text.
    pub fn remote(mut self) -> Self {
        self.remote = true;
        self
    }

    /// Reads its records from this and not from storage, for what no storage keeps.
    pub fn source(mut self, source: ListSource) -> Self {
        self.source = Some(source);
        self
    }

    fn reader(&self) -> Reader {
        Reader { session: self.session.clone(), frame: self.frame.clone(), cell_text: self.cell_text.clone() }
    }

    /// What this list asks of its records now.
    fn active_filter(&self) -> Option<Criteria> {
        both(self.filter.clone(), self.session.model_filter(&self.view.id))
    }

    fn load(
        &self,
        entity: &EntityInfo,
        fields: &[FieldInfo],
        filter: Option<&Criteria>,
        revision: u64,
        asked: String,
    ) -> Loaded {
        let space = self.frame.space();
        let mut query = Query::all();
        if let Some(filter) = filter {
            query = query.filter(filter.clone());
        }
        for (field, ascending) in &self.sort {
            query = query.sort(field.clone(), *ascending);
        }
        if self.access() == DataAccess::DataView {
            query = query.only(fields.iter().map(|field| field.name.clone()));
        }
        let source = self.source.clone().or_else(|| self.session.record_source(&entity.name));
        let found = match &source {
            Some(source) => Ok(source()),
            None => space.borrow().query(&entity.name, &query),
        };
        let objects = match found {
            Ok(objects) => objects,
            Err(error) => {
                self.session.set_error(Some(error.to_string()));
                Vec::new()
            }
        };
        let ordered = Rc::new(objects.clone());
        let key = entity.key_field().map(|f| f.name.clone()).unwrap_or_default();
        let rows = self.reader().rows(fields, &objects);
        let invalid = if self.edits_in_place(entity) {
            let words = &self.session.words().rules;
            let space = space.borrow();
            objects
                .iter()
                .flat_map(|object| {
                    let held = key_text(entity, &object.get(&key));
                    validate_in_place(entity, object, CONTEXT_SAVE, &*space, words)
                        .into_iter()
                        .filter(|violation| violation.is_error())
                        .filter_map(move |violation| Some((held.clone(), violation.field?)))
                        .collect::<Vec<_>>()
                })
                .collect()
        } else {
            BTreeSet::new()
        };
        let by_key =
            objects.into_iter().map(|object| (key_text(entity, &object.get(&key)), object)).collect();
        Loaded {
            revision,
            asked,
            data: GridData::new(rows),
            objects: Rc::new(by_key),
            ordered,
            invalid: Rc::new(invalid),
        }
    }

    /// How the model says this list gets its records.
    fn access(&self) -> DataAccess {
        let model = self.session.model();
        if self.remote {
            return DataAccess::Server;
        }
        model
            .at(&["Views", &self.view.id])
            .and_then(|node| node.text("DataAccess"))
            .map_or(DataAccess::Client, DataAccess::from_name)
    }

    /// Whether people change values right on this list: it has to hold all its records, whole.
    fn edits_in_place(&self, entity: &EntityInfo) -> bool {
        let model = self.session.model();
        let asked = model.at(&["Views", &self.view.id]).and_then(|node| node.flag("AllowEdit"));
        asked == Some(true)
            && self.access() == DataAccess::Client
            && self.source.is_none()
            && !entity.transient
    }

    /// Keeps only what still exists selected, as it is now, so actions judge it as it is.
    fn follow_selection(&self, chosen: &State<BTreeSet<String>>, objects: &BTreeMap<String, Object>) {
        let left: BTreeSet<String> =
            chosen.peek().iter().filter(|key| objects.contains_key(*key)).cloned().collect();
        if left.len() != chosen.peek().len() {
            chosen.set(left.clone());
        }
        let whole = self.access() != DataAccess::DataView;
        let fresh: Vec<Object> = left
            .iter()
            .filter_map(|key| {
                if whole {
                    return objects.get(key).cloned();
                }
                // A list that reads only its columns does not have what actions judge by.
                let (name, value) = objects.get(key).map(|object| (object.entity().to_owned(), key))?;
                let entity = self.session.entity(&name)?;
                let value = key_from_text(&entity, value);
                self.frame.space().borrow().find(&name, &value).ok().flatten()
            })
            .collect();
        if self.frame.selected() != fresh {
            self.frame.set_selected(fresh);
        }
    }

    fn remote_rows(
        &self,
        cx: &mut BuildCx<'_>,
        entity: &EntityInfo,
        fields: &[FieldInfo],
        filter: Option<&Criteria>,
        revision: u64,
        asked: &str,
    ) -> RemoteRows {
        let held: State<Option<Remote>> = cx.use_state(|| None);
        let kept = held.peek().clone().filter(|kept| kept.asked == asked);
        match kept {
            Some(kept) => {
                if kept.revision != revision {
                    kept.rows.refresh();
                    held.set(Some(Remote { revision, ..kept.clone() }));
                }
                kept.rows
            }
            None => {
                let (reader, entity, fields) = (self.reader(), entity.clone(), fields.to_vec());
                let (base, sort) = (filter.cloned(), self.sort.clone());
                let rows = RemoteRows::new(move |request, answer| {
                    answer_remote(&reader, &entity, &fields, base.as_ref(), &sort, &request, answer);
                });
                held.set(Some(Remote { asked: asked.to_owned(), revision, rows: rows.clone() }));
                rows
            }
        }
    }
}

impl View for ListView {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let session = &self.session;
        let model = session.model();
        let (Some(name), Some(node)) = (self.view.entity.as_deref(), model.at(&["Views", &self.view.id]))
        else {
            return cx.child(&Text::new(self.view.id.clone()));
        };
        let Some(entity) = session.entity(name) else {
            return cx.child(&Text::new(name.to_owned()));
        };
        let only = session.variant_columns(&self.view.id);
        let mut fields = shown_fields(&entity, node, only.as_deref());
        for (field, _, visible) in &mut fields {
            if self.implied.as_deref() == Some(field.name.as_str()) {
                *visible = false;
            }
        }
        let infos: Vec<FieldInfo> = fields.iter().map(|(field, ..)| (*field).clone()).collect();
        let revision = session.revision();
        let filter = self.active_filter();
        let asked =
            format!("{}|{:?}", filter.as_ref().map(ToString::to_string).unwrap_or_default(), self.sort);
        let chosen: State<BTreeSet<String>> = cx.use_state(BTreeSet::new);
        let editor_name = session
            .variant_editor(&self.view.id)
            .or_else(|| node.text("EditorType").map(str::to_owned))
            .or_else(|| {
                model.get("Views").and_then(|views| views.text("DefaultListEditor")).map(str::to_owned)
            });
        let editor = editor_name.as_deref().and_then(|name| session.list_editor(name));
        if let Some(editor) = editor {
            let loaded = self.load(&entity, &infos, filter.as_ref(), revision, asked);
            let (opener, open_entity, on_open) = (session.clone(), entity.clone(), self.on_open.clone());
            let frame = self.frame.clone();
            let request = ListEditorRequest {
                session: session.clone(),
                view: self.view.clone(),
                entity: entity.clone(),
                columns: fields
                    .iter()
                    .filter(|(_, _, visible)| *visible)
                    .map(|(field, caption, _)| ((*field).clone(), caption.clone()))
                    .collect(),
                objects: loaded.ordered,
                on_open: Rc::new(move |key: &Value| match &on_open {
                    Some(on_open) => on_open(key),
                    None => opener.open_detail(&open_entity.name, key),
                }),
                on_select: Rc::new(move |records| frame.set_selected(records)),
            };
            // Keyed, as what one way of showing keeps is not for another.
            let shown = editor(&request);
            let key = editor_name.as_deref().map_or(0, |name| {
                name.bytes()
                    .fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3))
            });
            return cx.keyed_child(key, shown.as_ref());
        }
        let access = self.access();
        let (data, objects, remote_rows, ordered, invalid) = if access == DataAccess::Server {
            let rows = self.remote_rows(cx, &entity, &infos, filter.as_ref(), revision, &asked);
            let none = (Rc::new(Vec::new()), Rc::new(BTreeSet::new()));
            (GridData::new(Vec::new()), Rc::new(BTreeMap::new()), Some(rows), none.0, none.1)
        } else {
            let loaded: State<Option<Loaded>> = cx.use_state(|| None);
            let kept = loaded.peek().clone().filter(|kept| kept.revision == revision && kept.asked == asked);
            let current = match kept {
                Some(kept) => kept,
                None => {
                    let fresh = self.load(&entity, &infos, filter.as_ref(), revision, asked);
                    self.follow_selection(&chosen, &fresh.objects);
                    loaded.set(Some(fresh.clone()));
                    fresh
                }
            };
            (current.data, current.objects, None, current.ordered, current.invalid)
        };
        let in_place = self.edits_in_place(&entity);
        let key_at = fields.iter().position(|(field, ..)| field.is_key).unwrap_or(0);
        let words = session.words();
        let default_property = entity.default_property().map(|f| f.name.clone());
        let names_the_record = |field: &FieldInfo| default_property.as_deref() == Some(field.name.as_str());
        let columns: Vec<GridColumn> = fields
            .iter()
            .map(|(field, caption, visible)| {
                let column = column_of(field, caption, words);
                // What names a record takes the room left over, so rows reach across.
                let column = if names_the_record(field) { column.fill() } else { column };
                // A column is for all its rows, so only a rule with no condition can hide it.
                let hidden =
                    entity.appearance_of(AppearanceTarget::Item, &field.name, &self.view, None).visible
                        == Some(false);
                let column = if *visible && !hidden { column } else { column.hidden() };
                let column = if in_place && editable_in_list(field) { column.editable() } else { column };
                match &self.column_shape {
                    Some(shape) => shape(field, column),
                    None => column,
                }
            })
            .collect();
        let selected = {
            let (read, write) = (chosen.clone(), chosen.clone());
            let (frame, known, found_in) = (self.frame.clone(), objects.clone(), entity.clone());
            // What holds only part of the records finds them when they are selected.
            let remote = remote_rows.is_some() || access == DataAccess::DataView;
            Binding::new(
                move || read.get(),
                move |keys: BTreeSet<String>| {
                    let records: Vec<Object> = keys
                        .iter()
                        .filter_map(|key| {
                            if !remote {
                                return known.get(key).cloned();
                            }
                            let value = key_from_text(&found_in, key);
                            frame.space().borrow().find(&found_in.name, &value).ok().flatten()
                        })
                        .collect();
                    frame.set_selected(records);
                    write.set(keys);
                },
            )
        };
        let (opener, open_entity, delete) = (session.clone(), entity.clone(), session.clone());
        let on_open = self.on_open.clone();
        let saver = cx.file_picker();
        let (exporter, caption) =
            (session.clone(), node.text("Caption").unwrap_or(&entity.caption).to_owned());
        let looks = (!entity.appearance.is_empty() || !invalid.is_empty()).then(|| {
            let names: Vec<String> = fields.iter().map(|(field, ..)| field.name.clone()).collect();
            let (rules, view, known) = (entity.clone(), self.view.clone(), objects.clone());
            let invalid = invalid.clone();
            move |row: &Row, column: usize| {
                let held = row.get(key_at).map(|cell| cell.key_text()).unwrap_or_default();
                let object = known.get(&held);
                let Some(name) = names.get(column) else { return GridCellStyle::default() };
                if invalid.contains(&(held, name.clone())) {
                    return GridCellStyle::new(Some(ravenrs_style::Semantic::Danger), None, true);
                }
                let found = rules.appearance_of(AppearanceTarget::Item, name, &view, object);
                GridCellStyle::new(found.font.map(semantic_of), found.back.map(semantic_of), found.bold)
            }
        });
        let current = cx.use_state(GridCurrent::new).get();
        if !self.compact {
            let culture = if words.decimal == ',' { "pt-BR" } else { "en-US" };
            let (current, title, columns) = (current.clone(), caption.clone(), columns.clone());
            let (reader, named, shown) = (self.reader(), entity.clone(), infos.clone());
            let (base, sort) = (filter.clone(), self.sort.clone());
            let report: ShownList = Rc::new(move || {
                if !current.is_remote() {
                    return current.table(&title, culture);
                }
                let asked = current.query()?;
                let rows = whole_remote(&reader, &named, &shown, base.as_ref(), &sort, &asked)
                    .map_err(|error| reader.session.set_error(Some(error)))
                    .ok()?;
                let every: Vec<usize> = (0..rows.len()).collect();
                Some(to_table(&title, culture, &columns, &rows, &asked, &every))
            });
            session.keep_shown_list(&self.view.id, report);
        }
        let grid = match remote_rows {
            Some(rows) => DataGrid::remote(rows, columns),
            None => DataGrid::new(data, columns),
        };
        let grid = match node.text("EmptyText") {
            Some(text) => grid.empty_text(text),
            None => grid,
        };
        let grid = grid
            .current(current)
            .striped(true)
            .key_column(key_at)
            .selection_mode(if self.single { SelectionMode::Single } else { SelectionMode::Multiple })
            .selected_keys(selected)
            .export_title(caption)
            .on_row_open(move |row| {
                if let Some(cell) = row.get(key_at) {
                    let key = key_from_text(&open_entity, &cell.key_text());
                    match &on_open {
                        Some(on_open) => on_open(&key),
                        None => opener.open_detail(&open_entity.name, &key),
                    }
                }
            })
            .on_delete_rows(move |_| {
                if let Some(action) = delete.frame().action(crate::ids::DELETE) {
                    delete.run_action(&action, ravenrs_app::ActionInput::None);
                }
            });
        let grid = if self.plain {
            grid
        } else {
            grid.filter_builder(true).on_export_file(move |format, table| {
                match export(&table, format, None) {
                    Ok(file) => exporter.hand_file(file.name, file.bytes, &saver),
                    Err(error) => exporter.set_error(Some(error)),
                }
            })
        };
        let grid = match looks {
            Some(looks) => grid.cell_style(looks),
            None => grid,
        };
        let grid = match default_property
            .as_deref()
            .and_then(|name| fields.iter().position(|(f, ..)| f.name == name))
        {
            Some(column) => grid.name_column(column),
            None => grid,
        };
        let grid = self.row_actions.iter().cloned().fold(grid, DataGrid::row_action);
        let grid = if in_place {
            let (session, frame, named, shown) =
                (session.clone(), self.frame.clone(), entity.clone(), infos.clone());
            grid.on_cell_edit(move |edit| edit_on_list(&session, &frame, &named, &shown, &ordered, edit))
        } else {
            grid
        };
        let grid = if self.compact {
            let height = objects.len().max(1) as f32 * 36.0 + 42.0;
            grid.toolbar(false)
                .filter_row(false)
                .footer(false)
                .selection_mode(SelectionMode::None)
                .height(height)
        } else {
            grid
        };
        let grid = match &self.grid_shape {
            Some(shape) => shape(grid),
            None => grid,
        };
        // Another set of columns starts over, since the grid keeps what it knows by position.
        match only {
            Some(names) => cx.child(&Keyed::new(hash_of(&names.join(",")), grid)),
            None => cx.child(&grid),
        }
    }
}

/// Keeps a value changed right on a list, in the record as storage has it now, if its rules let it
/// stand: a field already wrong elsewhere on the record does not stop it, and what only warns goes
/// on with a message.
fn edit_on_list(
    session: &Session,
    frame: &Frame,
    entity: &EntityInfo,
    fields: &[FieldInfo],
    ordered: &[Object],
    edit: &CellEdit,
) -> Result<(), String> {
    let words = session.words();
    let field = fields.get(edit.column).ok_or_else(|| words.rules.record.clone())?;
    let invalid = || words.rules.invalid.replace("{0}", &field.caption);
    let value = value_of_cell(field, &edit.value).ok_or_else(invalid)?;
    let key_name = entity.key_field().map(|key| key.name.clone()).unwrap_or_default();
    let key = ordered.get(edit.row).map(|object| object.get(&key_name)).ok_or_else(invalid)?;
    let space = frame.space();
    let stored = space.borrow().find(&entity.name, &key).map_err(|e| e.to_string())?;
    let mut record = stored.ok_or_else(|| words.record_gone.clone())?;
    record.set(&field.name, value);
    let found = session.violations_of(&record, CONTEXT_SAVE);
    let stops = |violation: &&ravenrs_app::Violation| {
        violation.is_error() && violation.field.as_ref().is_none_or(|name| *name == field.name)
    };
    if let Some(violation) = found.iter().find(stops) {
        return Err(violation.message.clone());
    }
    let kept = {
        let mut space = space.borrow_mut();
        space.save(record).and_then(|()| space.commit())
    };
    if let Err(error) = kept {
        space.borrow_mut().rollback();
        return Err(error.to_string());
    }
    for violation in found.iter().filter(|violation| violation.severity == Severity::Warning) {
        session.show_message(Message::new(violation.message.clone(), MessageKind::Warning));
    }
    session.touch();
    Ok(())
}

fn hash_of(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3))
}

#[cfg_attr(feature = "live", ravenrs_live::expose(part = "part1"))]
impl Session {
    /// The records of a list, the way it chose to see them, as this person can read them.
    pub fn list_records(&self, view: &ViewInfo) -> Vec<Object> {
        let Some(name) = view.entity.as_deref() else { return Vec::new() };
        let mut query = Query::all();
        if let Some(filter) = self.model_filter(&view.id) {
            query = query.filter(filter);
        }
        self.frame().space().borrow().query(name, &query).unwrap_or_default()
    }

    /// A list as it stands, as a table to make a report or a file from. While the list is on
    /// screen it is what the grid shows: the search, the filters of its columns and the filter
    /// builder, its sorting, the columns showing and its grouping, with every row that matches,
    /// on every page and in collapsed groups. A remote list asks storage again for all of them.
    /// Before the list is built, it is every record its model filter and variant let through.
    /// Either way the rows are read through the frame's storage, so only what the person can read.
    pub fn list_table(&self, view: &ViewInfo) -> Option<GridTable> {
        // A list on screen: what its filters, search, sorting and columns let through.
        if let Some(table) = self.shown_list(&view.id).and_then(|shown| shown()) {
            return Some(table);
        }
        let model = self.model();
        let (name, node) = (view.entity.as_deref()?, model.at(&["Views", &view.id])?);
        let entity = self.entity(name)?;
        let mut query = Query::all();
        if let Some(filter) = self.model_filter(&view.id) {
            query = query.filter(filter);
        }
        let objects = self.frame().space().borrow().query(name, &query).ok()?;
        let only = self.variant_columns(&view.id);
        let shown: Vec<(FieldInfo, String)> = shown_fields(&entity, node, only.as_deref())
            .into_iter()
            .filter(|(_, _, visible)| *visible)
            .map(|(field, caption, _)| (field.clone(), caption))
            .collect();
        let infos: Vec<FieldInfo> = shown.iter().map(|(field, _)| field.clone()).collect();
        let reader = Reader { session: self.clone(), frame: self.frame(), cell_text: None };
        let rows = reader.rows(&infos, &objects);
        let words = self.words();
        let columns: Vec<GridColumn> =
            shown.iter().map(|(field, caption)| column_of(field, caption, words)).collect();
        let culture = if words.decimal == ',' { "pt-BR" } else { "en-US" };
        let title = node.text("Caption").unwrap_or(&entity.caption);
        let order: Vec<usize> = (0..rows.len()).collect();
        Some(to_table(title, culture, &columns, &rows, &GridQuery::default(), &order))
    }
}
