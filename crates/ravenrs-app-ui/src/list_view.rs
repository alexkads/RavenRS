//! Many records: a [`DataGrid`] over the object space, shaped by the model.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use ravenrs_app::{
    AppearanceTarget, EntityInfo, FieldInfo, FieldKind, Frame, ModelNode, Object, Query, Value, ViewInfo,
};
use ravenrs_grid_export::export;
use ravenrs_reactive::{Binding, State};
use ravenrs_widgets::grid::{Row, Value as Cell};
use ravenrs_widgets::{
    BuildCx, DataGrid, GridCellStyle, GridColumn, GridData, NodeId, SelectionMode, Text, View,
};

use crate::session::Session;
use crate::values::{cell_of, column_of, display, key_from_text, key_text, semantic_of};

/// What opening a row of a list does, with its key.
pub type OpenHandler = Rc<dyn Fn(&Value)>;

/// A list read from storage, kept until something may have changed it.
#[derive(Clone)]
struct Loaded {
    revision: u64,
    data: GridData,
    objects: Rc<BTreeMap<String, Object>>,
}

/// The fields a list shows, in the order to show them, with the key last and out of sight, so
/// a row can always be found.
fn shown_fields<'a>(
    entity: &'a EntityInfo,
    view: &'a ModelNode,
) -> Vec<(&'a ravenrs_app::FieldInfo, String, bool)> {
    let mut fields: Vec<(&ravenrs_app::FieldInfo, String, bool)> = view
        .get("Columns")
        .map(|columns| columns.in_order())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|column| {
            let field = entity.field_named(&column.id)?;
            Some((field, column.text("Caption").unwrap_or(&field.caption).to_owned(), true))
        })
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
}

impl std::fmt::Debug for ListView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ListView").field("view", &self.view).finish_non_exhaustive()
    }
}

impl ListView {
    /// The list of this view in the model, reading through this frame.
    pub fn new(session: Session, frame: Frame, view: ViewInfo) -> Self {
        Self { session, frame, view, on_open: None }
    }

    /// What opening a row does, with its key. Its form by default.
    pub fn on_open(mut self, handler: OpenHandler) -> Self {
        self.on_open = Some(handler);
        self
    }

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

    fn load(
        &self,
        entity: &EntityInfo,
        fields: &[(&ravenrs_app::FieldInfo, String, bool)],
        revision: u64,
    ) -> Loaded {
        let space = self.frame.space();
        let found = space.borrow().query(&entity.name, &Query::all());
        let objects = match found {
            Ok(objects) => objects,
            Err(error) => {
                self.session.set_error(Some(error.to_string()));
                Vec::new()
            }
        };
        let key = entity.key_field().map(|f| f.name.clone()).unwrap_or_default();
        // A reference shows what names the record it points to, read once for the whole list.
        let names: BTreeMap<&str, BTreeMap<String, String>> = fields
            .iter()
            .filter_map(|(field, ..)| match &field.kind {
                FieldKind::Reference(target) => Some((field.name.as_str(), self.names_of(target))),
                _ => None,
            })
            .collect();
        let cell = |field: &FieldInfo, object: &Object| {
            let value = object.get(&field.name);
            match names.get(field.name.as_str()) {
                Some(known) if !matches!(value, Value::Null) => {
                    let text = display(field, &value);
                    Cell::Text(known.get(&text).cloned().unwrap_or(text))
                }
                _ => cell_of(field, &value),
            }
        };
        let rows = objects
            .iter()
            .map(|object| fields.iter().map(|(field, ..)| cell(field, object)).collect())
            .collect();
        let by_key =
            objects.into_iter().map(|object| (key_text(entity, &object.get(&key)), object)).collect();
        Loaded { revision, data: GridData::new(rows), objects: Rc::new(by_key) }
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
        let fields = shown_fields(&entity, node);
        let revision = session.revision();
        let loaded: State<Option<Loaded>> = cx.use_state(|| None);
        let chosen: State<BTreeSet<String>> = cx.use_state(BTreeSet::new);
        let current = {
            let kept = loaded.peek().clone().filter(|kept| kept.revision == revision);
            match kept {
                Some(kept) => kept,
                None => {
                    let fresh = self.load(&entity, &fields, revision);
                    if let Some(old) = loaded.peek().as_ref() {
                        // Whatever went away is no longer selected.
                        let left: BTreeSet<String> = chosen
                            .peek()
                            .iter()
                            .filter(|key| fresh.objects.contains_key(*key))
                            .cloned()
                            .collect();
                        if left.len() != chosen.peek().len() && old.revision != revision {
                            chosen.set(left);
                        }
                    }
                    loaded.set(Some(fresh.clone()));
                    fresh
                }
            }
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
                if *visible && !hidden { column } else { column.hidden() }
            })
            .collect();
        let selected = {
            let (read, write) = (chosen.clone(), chosen.clone());
            let (frame, objects) = (self.frame.clone(), current.objects.clone());
            Binding::new(
                move || read.get(),
                move |keys: BTreeSet<String>| {
                    frame.set_selected(keys.iter().filter_map(|key| objects.get(key).cloned()).collect());
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
            let (rules, view, objects) = (entity.clone(), self.view.clone(), current.objects.clone());
            move |row: &Row, column: usize| {
                let object = row.get(key_at).and_then(|cell| objects.get(&cell.key_text()));
                let Some(name) = names.get(column) else { return GridCellStyle::default() };
                let found = rules.appearance_of(AppearanceTarget::Item, name, &view, object);
                GridCellStyle::new(found.font.map(semantic_of), found.back.map(semantic_of), found.bold)
            }
        });
        let grid = DataGrid::new(current.data.clone(), columns)
            .key_column(key_at)
            .selection_mode(SelectionMode::Multiple)
            .selected_keys(selected)
            .filter_builder(true)
            .empty_text(caption.clone())
            .export_title(caption)
            .on_export_file(move |format, table| match export(&table, format, None) {
                Ok(file) => exporter.hand_file(file.name, file.bytes, &saver),
                Err(error) => exporter.set_error(Some(error)),
            })
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
        let grid = match looks {
            Some(looks) => grid.cell_style(looks),
            None => grid,
        };
        cx.child(&grid)
    }
}
