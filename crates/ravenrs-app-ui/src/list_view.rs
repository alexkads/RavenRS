//! Many records: a [`DataGrid`] over the object space, shaped by the model.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use ravenrs_app::{
    AppearanceTarget, Criteria, EntityInfo, FieldInfo, FieldKind, Frame, ModelNode, Object, Query, Value,
    ViewInfo,
};
use ravenrs_grid_export::export;
use ravenrs_reactive::{Binding, State};
use ravenrs_widgets::grid::{GridQuery, Row, Value as Cell, to_table};
use ravenrs_widgets::{
    AnyView, BuildCx, DataGrid, GridCellStyle, GridColumn, GridData, GridPage, GridRequest, GridResponder,
    GridRowAction, GridTable, Keyed, NodeId, RemoteRows, SelectionMode, SortDirection, Text, View,
};

use crate::session::Session;
use crate::values::{cell_of, column_of, display, key_from_text, key_text, semantic_of};

/// What opening a row of a list does, with its key.
pub type OpenHandler = Rc<dyn Fn(&Value)>;

/// What a list editor is asked to show: the records as the list reads them, and how to act on them.
#[non_exhaustive]
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
    let searchable =
        |field: &FieldInfo| matches!(field.kind, FieldKind::Text | FieldKind::LongText | FieldKind::Enum(_));
    let contains = |field: &FieldInfo, text: &str| {
        Criteria::parse(&format!("Contains(Upper([{}]), Upper({}))", field.name, literal(text))).ok()
    };
    let by_column = request.filters.iter().enumerate().fold(None, |all, (at, text)| {
        let this = fields
            .get(at)
            .filter(|field| searchable(field) && !text.trim().is_empty())
            .and_then(|field| contains(field, text.trim()));
        both(all, this)
    });
    let typed = request.search.trim();
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
            Criteria::parse(&format!("[{}] > {}", key.name, cell.key_text())).ok()
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
        let found = match &self.source {
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
        let by_key =
            objects.into_iter().map(|object| (key_text(entity, &object.get(&key)), object)).collect();
        Loaded { revision, asked, data: GridData::new(rows), objects: Rc::new(by_key), ordered }
    }

    /// Keeps only what still exists selected, as it is now, so actions judge it as it is.
    fn follow_selection(&self, chosen: &State<BTreeSet<String>>, objects: &BTreeMap<String, Object>) {
        let left: BTreeSet<String> =
            chosen.peek().iter().filter(|key| objects.contains_key(*key)).cloned().collect();
        if left.len() != chosen.peek().len() {
            chosen.set(left.clone());
        }
        let fresh: Vec<Object> = left.iter().filter_map(|key| objects.get(key).cloned()).collect();
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
        let fields = shown_fields(&entity, node, only.as_deref());
        let infos: Vec<FieldInfo> = fields.iter().map(|(field, ..)| (*field).clone()).collect();
        let revision = session.revision();
        let filter = self.active_filter();
        let asked =
            format!("{}|{:?}", filter.as_ref().map(ToString::to_string).unwrap_or_default(), self.sort);
        let chosen: State<BTreeSet<String>> = cx.use_state(BTreeSet::new);
        let editor = node
            .text("EditorType")
            .or_else(|| model.get("Views").and_then(|views| views.text("DefaultListEditor")))
            .and_then(|name| session.list_editor(name));
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
            return cx.child(&editor(&request));
        }
        let (data, objects, remote_rows) = if self.remote {
            let rows = self.remote_rows(cx, &entity, &infos, filter.as_ref(), revision, &asked);
            (GridData::new(Vec::new()), Rc::new(BTreeMap::new()), Some(rows))
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
            (current.data, current.objects, None)
        };
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
                match &self.column_shape {
                    Some(shape) => shape(field, column),
                    None => column,
                }
            })
            .collect();
        let selected = {
            let (read, write) = (chosen.clone(), chosen.clone());
            let (frame, known, found_in) = (self.frame.clone(), objects.clone(), entity.clone());
            let remote = remote_rows.is_some();
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
        let looks = (!entity.appearance.is_empty()).then(|| {
            let names: Vec<String> = fields.iter().map(|(field, ..)| field.name.clone()).collect();
            let (rules, view, known) = (entity.clone(), self.view.clone(), objects.clone());
            move |row: &Row, column: usize| {
                let object = row.get(key_at).and_then(|cell| known.get(&cell.key_text()));
                let Some(name) = names.get(column) else { return GridCellStyle::default() };
                let found = rules.appearance_of(AppearanceTarget::Item, name, &view, object);
                GridCellStyle::new(found.font.map(semantic_of), found.back.map(semantic_of), found.bold)
            }
        });
        let grid = match remote_rows {
            Some(rows) => DataGrid::remote(rows, columns),
            None => DataGrid::new(data, columns),
        };
        let grid = grid
            .key_column(key_at)
            .selection_mode(if self.single { SelectionMode::Single } else { SelectionMode::Multiple })
            .selected_keys(selected)
            .empty_text(caption.clone())
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

fn hash_of(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3))
}

impl Session {
    /// A list as it stands: the columns showing and every record the way it chose to see them, as
    /// a table to make a report or a file from.
    pub fn list_table(&self, view: &ViewInfo) -> Option<GridTable> {
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
