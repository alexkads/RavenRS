//! Reading and writing records through a unit of work.

use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;

use ravenrs_core::Storage;
use ravenrs_criteria::{Criteria, DateTime, Environment, Scope, Value};

use crate::{EntityInfo, FieldKind, ModelError, Object};

/// Why a space could not do what it was asked.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum SpaceError {
    /// There is no entity by this name.
    UnknownEntity(String),
    /// The filter or the sort does not fit the entity.
    BadQuery(String),
    /// The object has no key yet.
    MissingKey(String),
    /// Another record already has this key.
    DuplicateKey {
        /// The entity.
        entity: String,
        /// The key.
        key: String,
    },
    /// The record is gone, or something else took it since this space read it.
    Conflict(String),
    /// Something watching the commit refused it, with its reason.
    Refused(String),
    /// The person signed in cannot do this.
    Denied(String),
    /// Storage failed.
    Storage(String),
    /// What it needs is not there: a provider that does not do this, a feature that is off, a
    /// host that cannot. Trying again does not help until something changes.
    Unavailable(String),
    /// It failed this time and may work later: storage busy, a service out of reach.
    Temporary(String),
    /// It may have happened: there was no answer to tell. Find out before doing it again.
    Unknown(String),
}

impl fmt::Display for SpaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownEntity(name) => write!(f, "there is no entity named {name}"),
            Self::BadQuery(why)
            | Self::Conflict(why)
            | Self::Refused(why)
            | Self::Denied(why)
            | Self::Storage(why)
            | Self::Unavailable(why)
            | Self::Temporary(why)
            | Self::Unknown(why) => f.write_str(why),
            Self::MissingKey(entity) => write!(f, "a record of {entity} has no key"),
            Self::DuplicateKey { entity, key } => {
                write!(f, "{entity} already has a record with the key {key}")
            }
        }
    }
}

impl std::error::Error for SpaceError {}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl SpaceError {
    /// Whether trying the same thing again later may work.
    pub fn is_temporary(&self) -> bool {
        matches!(self, Self::Temporary(_))
    }

    /// Whether it may have happened in spite of the error, so that doing it again could do it twice.
    pub fn may_have_happened(&self) -> bool {
        matches!(self, Self::Unknown(_))
    }
}

/// What to read: a filter, an order and a page.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Query {
    /// Only the records this says true for.
    pub filter: Option<Criteria>,
    /// Fields to sort by, with true for ascending.
    pub sort: Vec<(String, bool)>,
    /// How many to skip.
    pub offset: usize,
    /// The most to return.
    pub limit: Option<usize>,
    /// Only these fields, with the key, for a list that shows few of many; none means every one.
    pub only: Option<Vec<String>>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Query {
    /// Everything, in key order.
    pub fn all() -> Self {
        Self::default()
    }

    /// Only the records this says true for.
    pub fn filter(mut self, filter: Criteria) -> Self {
        self.filter = Some(filter);
        self
    }

    /// Then by this field.
    pub fn sort(mut self, field: impl Into<String>, ascending: bool) -> Self {
        self.sort.push((field.into(), ascending));
        self
    }

    /// Only these fields, with the key. What is read this way is not for writing back.
    pub fn only(mut self, fields: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.only = Some(fields.into_iter().map(Into::into).collect());
        self
    }

    /// A page of these many, after skipping those.
    pub fn page(mut self, offset: usize, limit: usize) -> Self {
        self.offset = offset;
        self.limit = Some(limit);
        self
    }
}

/// A change waiting for a commit.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum Change {
    /// A new record.
    New(Object),
    /// A record that changed.
    Updated(Object),
    /// A record to take away, by its entity and key.
    Deleted {
        /// The entity.
        entity: String,
        /// The key.
        key: Value,
    },
}

/// What happens to a space, for whoever watches it.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub enum SpaceEvent<'a> {
    /// A commit is about to happen. An error stops it, and nothing is kept.
    Committing(&'a [Change]),
    /// A commit happened.
    Committed(&'a [Change]),
    /// The changes were thrown away.
    RolledBack,
}

/// Watches a space; see [`ObjectSpace::on_event`].
pub type SpaceWatcher = Box<dyn FnMut(SpaceEvent<'_>) -> Result<(), String>>;

/// A unit of work over records: what it reads includes its own changes, and nothing
/// reaches storage until [`commit`](Self::commit). A view has one of these, so canceling a
/// form throws away only that form's changes.
pub trait ObjectSpace {
    /// The entities it knows.
    fn entities(&self) -> Vec<EntityInfo>;
    /// Reads records.
    fn query(&self, entity: &str, query: &Query) -> Result<Vec<Object>, SpaceError>;
    /// Reads one record by its key.
    fn find(&self, entity: &str, key: &Value) -> Result<Option<Object>, SpaceError>;
    /// A record with what a new one starts with, kept only once [`save`](Self::save)d.
    fn new_object(&mut self, entity: &str) -> Result<Object, SpaceError>;
    /// Marks a record as new or changed, by its key.
    fn save(&mut self, object: Object) -> Result<(), SpaceError>;
    /// Marks a record as new, refusing a key already in use, as a form creating
    /// a record has to: it must not take over the record with that key.
    fn insert(&mut self, object: Object) -> Result<(), SpaceError>;
    /// Marks a record to go away.
    fn delete(&mut self, entity: &str, key: &Value) -> Result<(), SpaceError>;
    /// What waits for a commit.
    fn changes(&self) -> Vec<Change>;
    /// Whether anything waits for a commit.
    fn is_modified(&self) -> bool {
        !self.changes().is_empty()
    }
    /// Keeps every change, or none of them.
    fn commit(&mut self) -> Result<(), SpaceError>;
    /// Throws away every change.
    fn rollback(&mut self);
    /// Throws away every change and reads again from storage.
    fn refresh(&mut self) {
        self.rollback();
    }
    /// Calls this when something happens to the space.
    fn on_event(&mut self, watcher: SpaceWatcher);
}

impl ObjectSpace for Box<dyn ObjectSpace> {
    fn entities(&self) -> Vec<EntityInfo> {
        (**self).entities()
    }
    fn query(&self, entity: &str, query: &Query) -> Result<Vec<Object>, SpaceError> {
        (**self).query(entity, query)
    }
    fn find(&self, entity: &str, key: &Value) -> Result<Option<Object>, SpaceError> {
        (**self).find(entity, key)
    }
    fn new_object(&mut self, entity: &str) -> Result<Object, SpaceError> {
        (**self).new_object(entity)
    }
    fn save(&mut self, object: Object) -> Result<(), SpaceError> {
        (**self).save(object)
    }
    fn insert(&mut self, object: Object) -> Result<(), SpaceError> {
        (**self).insert(object)
    }
    fn delete(&mut self, entity: &str, key: &Value) -> Result<(), SpaceError> {
        (**self).delete(entity, key)
    }
    fn changes(&self) -> Vec<Change> {
        (**self).changes()
    }
    fn is_modified(&self) -> bool {
        (**self).is_modified()
    }
    fn commit(&mut self) -> Result<(), SpaceError> {
        (**self).commit()
    }
    fn rollback(&mut self) {
        (**self).rollback();
    }
    fn refresh(&mut self) {
        (**self).refresh();
    }
    fn on_event(&mut self, watcher: SpaceWatcher) {
        (**self).on_event(watcher);
    }
}

/// A key as text, the same for equal keys.
pub fn key_text(key: &Value) -> Option<String> {
    match key {
        Value::Null => None,
        Value::Text(text) if text.is_empty() => None,
        Value::Text(text) => Some(text.clone()),
        Value::Number(number) if number.fract() == 0.0 => Some(format!("{}", *number as i64)),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(on) => Some(on.to_string()),
        Value::Date(date) => Some(format!("{date:?}")),
        _ => None,
    }
}

fn compare(a: &Value, b: &Value) -> Ordering {
    match (a, b) {
        (Value::Null, Value::Null) => Ordering::Equal,
        (Value::Null, _) => Ordering::Less,
        (_, Value::Null) => Ordering::Greater,
        (Value::Number(x), Value::Number(y)) => x.total_cmp(y),
        (Value::Text(x), Value::Text(y)) => x.cmp(y),
        (Value::Bool(x), Value::Bool(y)) => x.cmp(y),
        (Value::Date(x), Value::Date(y)) => (x.days, x.seconds).cmp(&(y.days, y.seconds)),
        // Different kinds in one field come in the order of their kinds: sorting needs an order
        // that holds across all of them, and panics on one that does not.
        _ => kind_rank(a).cmp(&kind_rank(b)),
    }
}

fn kind_rank(value: &Value) -> u8 {
    match value {
        Value::Null => 0,
        Value::Bool(_) => 1,
        Value::Number(_) => 2,
        Value::Text(_) => 3,
        Value::Date(_) => 4,
        _ => 5,
    }
}

/// The key of a record, as text.
pub fn key_of(entity: &EntityInfo, object: &Object) -> Option<String> {
    entity.key_field().and_then(|field| key_text(&object.get(&field.name)))
}

/// What a unit of work has waiting, for whoever implements one: new, changed and deleted
/// records, one change per record.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct ChangeSet {
    changes: Vec<Change>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl ChangeSet {
    /// What waits, in the order it came.
    pub fn list(&self) -> &[Change] {
        &self.changes
    }

    /// Throws it all away.
    pub fn clear(&mut self) {
        self.changes.clear();
    }

    /// Whether anything waits for this entity.
    pub fn touches(&self, entity: &str) -> bool {
        self.changes.iter().any(|change| match change {
            Change::New(object) | Change::Updated(object) => object.entity() == entity,
            Change::Deleted { entity: name, .. } => name == entity,
        })
    }

    fn position(&self, entity: &EntityInfo, key: &str) -> Option<usize> {
        self.changes.iter().position(|change| match change {
            Change::New(object) | Change::Updated(object) => {
                object.entity() == entity.name && key_of(entity, object).as_deref() == Some(key)
            }
            Change::Deleted { entity: name, key: deleted } => {
                *name == entity.name && key_text(deleted).as_deref() == Some(key)
            }
        })
    }

    /// Marks a record as new or changed. `exists` says whether storage has its key.
    pub fn save(&mut self, entity: &EntityInfo, mut object: Object, exists: bool) -> Result<(), SpaceError> {
        entity.strip_calculated(&mut object);
        let key = key_of(entity, &object).ok_or_else(|| SpaceError::MissingKey(entity.name.clone()))?;
        match self.position(entity, &key) {
            Some(at) if matches!(self.changes[at], Change::New(_)) => self.changes[at] = Change::New(object),
            Some(at) => self.changes[at] = Change::Updated(object),
            None if exists => self.changes.push(Change::Updated(object)),
            None => self.changes.push(Change::New(object)),
        }
        Ok(())
    }

    /// Marks a record as new, refusing a key in use. `exists` says whether storage has it.
    pub fn insert(
        &mut self,
        entity: &EntityInfo,
        mut object: Object,
        exists: bool,
    ) -> Result<(), SpaceError> {
        entity.strip_calculated(&mut object);
        let key = key_of(entity, &object).ok_or_else(|| SpaceError::MissingKey(entity.name.clone()))?;
        let waiting = self.position(entity, &key);
        let taken = match waiting {
            Some(at) => !matches!(self.changes[at], Change::Deleted { .. }),
            None => exists,
        };
        if taken {
            return Err(SpaceError::DuplicateKey { entity: entity.name.clone(), key });
        }
        match waiting {
            // What was going away comes back as a change to it.
            Some(at) => self.changes[at] = Change::Updated(object),
            None => self.changes.push(Change::New(object)),
        }
        Ok(())
    }

    /// Marks a record to go away. `exists` says whether storage has it.
    pub fn delete(&mut self, entity: &EntityInfo, key: &Value, exists: bool) {
        let Some(text) = key_text(key) else { return };
        match self.position(entity, &text) {
            Some(at) if matches!(self.changes[at], Change::New(_)) => {
                self.changes.remove(at);
            }
            Some(at) => {
                self.changes[at] = Change::Deleted { entity: entity.name.clone(), key: key.clone() };
            }
            None if exists => {
                self.changes.push(Change::Deleted { entity: entity.name.clone(), key: key.clone() });
            }
            None => {}
        }
    }

    /// Puts what waits on top of what storage has, by key.
    pub fn overlay(&self, entity: &EntityInfo, rows: &mut BTreeMap<String, Object>) {
        for change in &self.changes {
            match change {
                Change::New(object) | Change::Updated(object) if object.entity() == entity.name => {
                    if let Some(key) = key_of(entity, object) {
                        rows.insert(key, object.clone());
                    }
                }
                Change::Deleted { entity: name, key } if *name == entity.name => {
                    if let Some(key) = key_text(key) {
                        rows.remove(&key);
                    }
                }
                _ => {}
            }
        }
    }
}

/// Says whether a query fits an entity.
pub fn check_query(entity: &EntityInfo, query: &Query) -> Result<(), SpaceError> {
    if let Some(filter) = &query.filter {
        let environment = crate::environment();
        let problems = ravenrs_criteria::validate(filter, &entity.schema());
        // A function the application added is known only where it runs.
        let found = problems.iter().find(|problem| {
            !matches!(problem, ravenrs_criteria::Problem::UnknownFunction(name) if environment.has_function(name))
        });
        if let Some(problem) = found {
            return Err(SpaceError::BadQuery(format!("{}: {problem}", entity.name)));
        }
    }
    for (field, _) in &query.sort {
        if entity.field_named(field).is_none() {
            return Err(SpaceError::BadQuery(format!("{} has no field {field}", entity.name)));
        }
    }
    Ok(())
}

/// Runs a query over records in memory: computes, filters, sorts and takes the page.
pub fn run_query(
    entity: &EntityInfo,
    objects: Vec<Object>,
    query: &Query,
    environment: &Environment,
) -> Result<Vec<Object>, SpaceError> {
    check_query(entity, query)?;
    let objects = objects
        .into_iter()
        .map(|mut object| {
            entity.compute(&mut object, environment);
            object
        })
        .collect();
    select(entity, objects, query, environment)
}

/// Filters, sorts and takes the page of records as they are, computed or not.
pub(crate) fn select(
    entity: &EntityInfo,
    objects: Vec<Object>,
    query: &Query,
    environment: &Environment,
) -> Result<Vec<Object>, SpaceError> {
    let key_field = entity.key_field().map(|f| f.name.clone()).unwrap_or_default();
    let mut found: Vec<Object> = objects
        .into_iter()
        .filter(|object| {
            query.filter.as_ref().is_none_or(|filter| Scope::new(environment).row(object).matches(filter))
        })
        .collect();
    found.sort_by(|a, b| {
        query
            .sort
            .iter()
            .map(|(field, ascending)| {
                let order = compare(&a.get(field), &b.get(field));
                if *ascending { order } else { order.reverse() }
            })
            .find(|order| order.is_ne())
            .unwrap_or_else(|| compare(&a.get(&key_field), &b.get(&key_field)))
    });
    let page = found.into_iter().skip(query.offset);
    let mut chosen: Vec<Object> = match query.limit {
        Some(limit) => page.take(limit).collect(),
        None => page.collect(),
    };
    if let Some(only) = &query.only {
        let mut names: Vec<&str> = only.iter().map(String::as_str).collect();
        names.push(&key_field);
        for object in &mut chosen {
            object.keep_only(&names);
        }
    }
    Ok(chosen)
}

/// What comes of putting together what a person changed and what someone else kept meanwhile.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Merged {
    /// The record with both sets of changes. On a field both changed, it has this person's.
    pub record: Object,
    /// The fields both changed, each to something else.
    pub conflicts: Vec<String>,
}

/// Puts together what a person changed, from `original` to `mine`, and what is `stored` now.
/// Changes to different fields both stand, so only changing the same field takes a decision.
pub fn merge_changes(entity: &EntityInfo, original: &Object, mine: &Object, stored: &Object) -> Merged {
    let mut record = stored.clone();
    let mut conflicts = Vec::new();
    let fields = entity.fields.iter().filter(|field| {
        !field.is_key && field.calculated.is_none() && !matches!(field.kind, FieldKind::Collection(_))
    });
    for field in fields {
        let (was, changed, now) = (original.get(&field.name), mine.get(&field.name), stored.get(&field.name));
        if changed == was {
            continue;
        }
        if now != was && now != changed {
            conflicts.push(field.name.clone());
        }
        record.set(&field.name, changed);
    }
    Merged { record, conflicts }
}

#[derive(Debug, Default)]
struct Tables {
    rows: BTreeMap<String, BTreeMap<String, Object>>,
    next_key: i64,
}

/// Where a [`MemoryDatabase`] keeps its records, and what went wrong the last time it tried.
struct Kept {
    storage: Rc<dyn Storage>,
    name: String,
    problem: Option<String>,
}

/// Records kept in memory, on desktop and in the browser. Any number of spaces share them, and
/// each sees what the others committed.
///
/// With [`Self::stored_in`] they are also kept in a [`Storage`] between runs: read when it
/// opens, written after every commit. [`Self::export_json`] and [`Self::import_json`] move all
/// of them in and out at once.
#[derive(Clone)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct MemoryDatabase {
    entities: Rc<Vec<EntityInfo>>,
    tables: Rc<RefCell<Tables>>,
    kept: Rc<RefCell<Option<Kept>>>,
    restored: bool,
}

impl fmt::Debug for MemoryDatabase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MemoryDatabase")
            .field("entities", &self.entities.len())
            .field("kept", &self.kept.borrow().as_ref().map(|k| k.name.clone()))
            .finish_non_exhaustive()
    }
}

/// The version of [`MemoryDatabase::export_json`]'s format.
const RECORDS_FORMAT: u64 = 1;

fn value_to_json(value: &Value) -> serde_json::Value {
    match value {
        Value::Null => serde_json::Value::Null,
        Value::Bool(b) => serde_json::Value::Bool(*b),
        Value::Number(n) => {
            serde_json::Number::from_f64(*n).map_or(serde_json::Value::Null, serde_json::Value::Number)
        }
        Value::Text(text) => serde_json::Value::String(text.clone()),
        Value::Date(moment) => serde_json::json!({ "days": moment.days, "seconds": moment.seconds }),
        // A kind of value newer than this format is left out rather than misread later.
        _ => serde_json::Value::Null,
    }
}

fn value_from_json(json: &serde_json::Value) -> Result<Value, String> {
    Ok(match json {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Number(n) => Value::Number(n.as_f64().ok_or("a number out of range")?),
        serde_json::Value::String(text) => Value::Text(text.clone()),
        serde_json::Value::Object(moment) => {
            let days = moment.get("days").and_then(serde_json::Value::as_i64).ok_or("a date without days")?;
            let seconds = moment.get("seconds").and_then(serde_json::Value::as_u64).unwrap_or(0);
            Value::Date(DateTime {
                days,
                seconds: u32::try_from(seconds).map_err(|_| "seconds out of range")?,
            })
        }
        serde_json::Value::Array(_) => return Err("a list where a value goes".to_owned()),
    })
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl MemoryDatabase {
    /// A database with these entities and no records.
    pub fn new(entities: Vec<EntityInfo>) -> Result<Self, ModelError> {
        for entity in &entities {
            entity.check()?;
        }
        Ok(Self {
            entities: Rc::new(entities),
            tables: Rc::new(RefCell::new(Tables { next_key: 1, ..Tables::default() })),
            kept: Rc::new(RefCell::new(None)),
            restored: false,
        })
    }

    /// Keeps the records in `storage` under `name`: what is there is read now, and every commit
    /// writes them all back. Records of entities the database no longer has are left out.
    pub fn stored_in(
        mut self,
        storage: impl Storage + 'static,
        name: impl Into<String>,
    ) -> Result<Self, SpaceError> {
        let name = name.into();
        if let Some(bytes) = storage.read(&name).map_err(SpaceError::Storage)? {
            let text = String::from_utf8(bytes).map_err(|e| SpaceError::Storage(format!("{name}: {e}")))?;
            self.load(&text, false).map_err(|e| SpaceError::Storage(format!("{name}: {e}")))?;
            self.restored = true;
        }
        *self.kept.borrow_mut() = Some(Kept { storage: Rc::new(storage), name, problem: None });
        Ok(self)
    }

    /// Whether [`Self::stored_in`] found records kept from before, so an app can put its first
    /// records in only once.
    pub fn was_restored(&self) -> bool {
        self.restored
    }

    /// What went wrong the last time the records were written to their storage, if it did.
    pub fn storage_problem(&self) -> Option<String> {
        self.kept.borrow().as_ref().and_then(|kept| kept.problem.clone())
    }

    /// Every record, as JSON: `{"format": 1, "next_key": 7, "records": {"Account": [{...}]}}`.
    /// Dates are `{"days": .., "seconds": ..}`; the rest are plain JSON values.
    pub fn export_json(&self) -> String {
        let tables = self.tables.borrow();
        let records: serde_json::Map<String, serde_json::Value> = tables
            .rows
            .iter()
            .map(|(entity, rows)| {
                let rows = rows
                    .values()
                    .map(|object| {
                        let fields =
                            object.values().iter().map(|(k, v)| (k.clone(), value_to_json(v))).collect();
                        serde_json::Value::Object(fields)
                    })
                    .collect();
                (entity.clone(), serde_json::Value::Array(rows))
            })
            .collect();
        let whole = serde_json::json!({
            "format": RECORDS_FORMAT,
            "next_key": tables.next_key,
            "records": records,
        });
        serde_json::to_string_pretty(&whole).unwrap_or_else(|_| "{}".to_owned())
    }

    /// Puts these records, as [`Self::export_json`] writes them, in place of all there are, and
    /// keeps them if the database is stored. Returns how many there are. Nothing changes if
    /// anything is wrong: an entity it does not have, a record without its key, a value that
    /// cannot be read.
    pub fn import_json(&self, text: &str) -> Result<usize, SpaceError> {
        let count = self.load(text, true).map_err(SpaceError::Storage)?;
        self.persist();
        Ok(count)
    }

    /// Reads records into the tables, all or nothing. Strict refuses entities it does not have.
    fn load(&self, text: &str, strict: bool) -> Result<usize, String> {
        let whole: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let format = whole.get("format").and_then(serde_json::Value::as_u64).unwrap_or(RECORDS_FORMAT);
        if format > RECORDS_FORMAT {
            return Err(format!("records written by a newer version (format {format})"));
        }
        let mut rows: BTreeMap<String, BTreeMap<String, Object>> = BTreeMap::new();
        let mut next_key = whole.get("next_key").and_then(serde_json::Value::as_i64).unwrap_or(1).max(1);
        let empty = serde_json::Map::new();
        let records = whole.get("records").and_then(serde_json::Value::as_object).unwrap_or(&empty);
        for (name, list) in records {
            let Some(entity) = self.entities.iter().find(|e| &e.name == name) else {
                if strict {
                    return Err(format!("there is no entity named {name}"));
                }
                continue;
            };
            let list = list.as_array().ok_or_else(|| format!("{name}: a list of records goes here"))?;
            let table = rows.entry(name.clone()).or_default();
            for record in list {
                let fields = record.as_object().ok_or_else(|| format!("{name}: a record is an object"))?;
                let mut values = BTreeMap::new();
                for (field, value) in fields {
                    values.insert(
                        field.clone(),
                        value_from_json(value).map_err(|e| format!("{name}.{field}: {e}"))?,
                    );
                }
                let object = Object::from_values(name.clone(), values);
                let key =
                    key_of(entity, &object).ok_or_else(|| format!("{name}: a record without its key"))?;
                if let Some(field) = entity.key_field()
                    && matches!(field.kind, FieldKind::Integer)
                    && let Value::Number(n) = object.get(&field.name)
                {
                    next_key = next_key.max(n as i64 + 1);
                }
                table.insert(key, object);
            }
        }
        let count = rows.values().map(BTreeMap::len).sum();
        let mut tables = self.tables.borrow_mut();
        tables.rows = rows;
        tables.next_key = next_key;
        Ok(count)
    }

    /// Writes every record to the storage, if there is one, and remembers what went wrong.
    fn persist(&self) {
        let text = if self.kept.borrow().is_some() { self.export_json() } else { return };
        if let Some(kept) = self.kept.borrow_mut().as_mut() {
            kept.problem = kept.storage.write(&kept.name, text.as_bytes()).err();
        }
    }

    /// Puts these in place of the records of an entity, with no rules or states to go through, for
    /// a copy of what is kept somewhere else.
    pub fn replace_records(&self, entity: &str, records: Vec<Object>) {
        let Some(info) = self.entities.iter().find(|known| known.name == entity) else { return };
        let mut tables = self.tables.borrow_mut();
        let highest = records
            .iter()
            .filter_map(|record| match info.key_field().map(|field| record.get(&field.name)) {
                Some(Value::Number(number)) => Some(number as i64),
                _ => None,
            })
            .max()
            .unwrap_or(0);
        tables.next_key = tables.next_key.max(highest + 1);
        let rows =
            records.into_iter().filter_map(|record| key_of(info, &record).map(|key| (key, record))).collect();
        tables.rows.insert(entity.to_owned(), rows);
    }

    /// A new unit of work over these records.
    pub fn space(&self) -> MemoryObjectSpace {
        MemoryObjectSpace {
            database: self.clone(),
            environment: crate::environment(),
            changes: ChangeSet::default(),
            watchers: Vec::new(),
        }
    }
}

/// A unit of work over a [`MemoryDatabase`].
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct MemoryObjectSpace {
    database: MemoryDatabase,
    environment: Environment,
    changes: ChangeSet,
    watchers: Vec<SpaceWatcher>,
}

impl fmt::Debug for MemoryObjectSpace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MemoryObjectSpace").field("changes", &self.changes).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl MemoryObjectSpace {
    fn entity(&self, name: &str) -> Result<&EntityInfo, SpaceError> {
        self.database
            .entities
            .iter()
            .find(|e| e.name == name)
            .ok_or_else(|| SpaceError::UnknownEntity(name.to_owned()))
    }

    fn committed_has(&self, entity: &str, key: &str) -> bool {
        self.database.tables.borrow().rows.get(entity).is_some_and(|table| table.contains_key(key))
    }

    /// Stored records with this space's changes on top, by key.
    fn current(&self, entity: &EntityInfo) -> BTreeMap<String, Object> {
        let mut rows = self.database.tables.borrow().rows.get(&entity.name).cloned().unwrap_or_default();
        self.changes.overlay(entity, &mut rows);
        rows
    }

    fn tell(&mut self, event: SpaceEvent<'_>) -> Result<(), SpaceError> {
        for watcher in &mut self.watchers {
            watcher(event).map_err(SpaceError::Refused)?;
        }
        Ok(())
    }

    /// Says what would stop these changes, before any is applied.
    fn check_changes(&self) -> Result<(), SpaceError> {
        for change in self.changes.list() {
            match change {
                Change::New(object) => {
                    let entity = self.entity(object.entity())?;
                    let key =
                        key_of(entity, object).ok_or_else(|| SpaceError::MissingKey(entity.name.clone()))?;
                    if self.committed_has(&entity.name, &key) {
                        return Err(SpaceError::DuplicateKey { entity: entity.name.clone(), key });
                    }
                    crate::check_transitions(entity, None, object)?;
                }
                Change::Updated(object) => {
                    let entity = self.entity(object.entity())?;
                    let key =
                        key_of(entity, object).ok_or_else(|| SpaceError::MissingKey(entity.name.clone()))?;
                    if !self.committed_has(&entity.name, &key) {
                        return Err(SpaceError::Conflict(format!(
                            "the record {key} of {} is gone",
                            entity.name
                        )));
                    }
                    let tables = self.database.tables.borrow();
                    let stored = tables.rows.get(&entity.name).and_then(|rows| rows.get(&key));
                    crate::check_transitions(entity, stored, object)?;
                }
                Change::Deleted { .. } => {}
            }
        }
        Ok(())
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose(part = "objectspace"))]
impl ObjectSpace for MemoryObjectSpace {
    fn entities(&self) -> Vec<EntityInfo> {
        self.database.entities.as_ref().clone()
    }

    fn query(&self, entity: &str, query: &Query) -> Result<Vec<Object>, SpaceError> {
        let info = self.entity(entity)?;
        let objects = self.current(info).into_values().collect();
        run_query(info, objects, query, &self.environment)
    }

    fn find(&self, entity: &str, key: &Value) -> Result<Option<Object>, SpaceError> {
        let info = self.entity(entity)?;
        let Some(key) = key_text(key) else { return Ok(None) };
        Ok(self.current(info).remove(&key).map(|mut object| {
            info.compute(&mut object, &self.environment);
            object
        }))
    }

    fn new_object(&mut self, entity: &str) -> Result<Object, SpaceError> {
        let info = self.entity(entity)?;
        let mut object = info.new_object();
        // Numbered keys are given here, and never again, even if this record is not kept.
        if let Some(field) = info.key_field()
            && matches!(field.kind, FieldKind::Integer)
            && matches!(object.get(&field.name), Value::Null)
        {
            let mut tables = self.database.tables.borrow_mut();
            object.set(&field.name, Value::Number(tables.next_key as f64));
            tables.next_key += 1;
        }
        Ok(object)
    }

    fn save(&mut self, object: Object) -> Result<(), SpaceError> {
        let info = self.entity(object.entity())?.clone();
        let exists = key_of(&info, &object).is_some_and(|key| self.committed_has(&info.name, &key));
        self.changes.save(&info, object, exists)
    }

    fn insert(&mut self, object: Object) -> Result<(), SpaceError> {
        let info = self.entity(object.entity())?.clone();
        let exists = key_of(&info, &object).is_some_and(|key| self.committed_has(&info.name, &key));
        self.changes.insert(&info, object, exists)
    }

    fn delete(&mut self, entity: &str, key: &Value) -> Result<(), SpaceError> {
        let info = self.entity(entity)?.clone();
        let exists = key_text(key).is_some_and(|text| self.committed_has(&info.name, &text));
        self.changes.delete(&info, key, exists);
        Ok(())
    }

    fn changes(&self) -> Vec<Change> {
        self.changes.list().to_vec()
    }

    fn commit(&mut self) -> Result<(), SpaceError> {
        self.check_changes()?;
        let changes = self.changes.list().to_vec();
        self.tell(SpaceEvent::Committing(&changes))?;
        // Decimals are kept rounded to their places, as every database keeps them.
        let mut kept = changes.clone();
        for change in &mut kept {
            if let Change::New(object) | Change::Updated(object) = change {
                let entity = self.entity(object.entity())?;
                crate::exact_decimals(entity, object).map_err(SpaceError::Refused)?;
            }
        }
        {
            let mut tables = self.database.tables.borrow_mut();
            for change in &kept {
                match change {
                    Change::New(object) | Change::Updated(object) => {
                        let info = self.database.entities.iter().find(|e| e.name == object.entity());
                        if let Some(key) = info.and_then(|info| key_of(info, object)) {
                            tables
                                .rows
                                .entry(object.entity().to_owned())
                                .or_default()
                                .insert(key, object.clone());
                        }
                    }
                    Change::Deleted { entity, key } => {
                        if let (Some(table), Some(key)) = (tables.rows.get_mut(entity), key_text(key)) {
                            table.remove(&key);
                        }
                    }
                }
            }
        }
        self.changes.clear();
        self.database.persist();
        // The commit stands; someone watching after it cannot undo it.
        let _ = self.tell(SpaceEvent::Committed(&changes));
        Ok(())
    }

    fn rollback(&mut self) {
        self.changes.clear();
        let _ = self.tell(SpaceEvent::RolledBack);
    }

    fn on_event(&mut self, watcher: SpaceWatcher) {
        self.watchers.push(watcher);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FieldInfo;

    fn database() -> MemoryDatabase {
        MemoryDatabase::new(vec![
            EntityInfo::new("Account")
                .field(FieldInfo::new("id", FieldKind::Integer).key())
                .field(FieldInfo::new("name", FieldKind::Text).default_value("Nova"))
                .field(FieldInfo::new("cents", FieldKind::Money).default_value(0.0))
                .field(FieldInfo::new("double", FieldKind::Money).calculated("[cents] * 2")),
            EntityInfo::new("Bank")
                .field(FieldInfo::new("code", FieldKind::Text).typed_key())
                .field(FieldInfo::new("name", FieldKind::Text)),
        ])
        .expect("valid")
    }

    fn add(space: &mut MemoryObjectSpace, name: &str, cents: f64) -> Value {
        let mut object = space.new_object("Account").expect("the entity");
        object.set("name", name);
        object.set("cents", cents);
        let key = object.get("id");
        space.save(object).expect("saves");
        key
    }

    fn names(space: &MemoryObjectSpace, query: &Query) -> Vec<String> {
        let found = space.query("Account", query).expect("reads");
        found.iter().map(|o| if let Value::Text(t) = o.get("name") { t } else { String::new() }).collect()
    }

    #[test]
    fn nothing_is_kept_until_the_commit_and_the_space_sees_its_own_changes() {
        let db = database();
        let (mut mine, other) = (db.space(), db.space());
        add(&mut mine, "A", 1.0);
        assert!(mine.is_modified());
        assert_eq!(names(&mine, &Query::all()), ["A"]);
        assert!(names(&other, &Query::all()).is_empty());
        mine.commit().expect("commits");
        assert!(!mine.is_modified());
        assert_eq!(names(&other, &Query::all()), ["A"]);
    }

    #[test]
    fn a_rollback_throws_away_new_changed_and_deleted_records() {
        let db = database();
        let mut space = db.space();
        let (a, b) = (add(&mut space, "A", 1.0), add(&mut space, "B", 2.0));
        space.commit().expect("commits");
        let mut changed = space.find("Account", &a).expect("reads").expect("there");
        changed.set("name", "A2");
        space.save(changed).expect("saves");
        space.delete("Account", &b).expect("deletes");
        add(&mut space, "C", 3.0);
        assert_eq!(names(&space, &Query::all()), ["A2", "C"]);
        assert_eq!(space.changes().len(), 3);
        space.rollback();
        assert_eq!(names(&space, &Query::all()), ["A", "B"]);
    }

    #[test]
    fn a_new_record_deleted_before_the_commit_was_never_there() {
        let db = database();
        let mut space = db.space();
        let key = add(&mut space, "A", 1.0);
        space.delete("Account", &key).expect("deletes");
        assert!(!space.is_modified());
        let again = add(&mut space, "B", 1.0);
        let mut object = space.find("Account", &again).expect("reads").expect("there");
        object.set("name", "B2");
        space.save(object).expect("saves");
        assert!(matches!(space.changes().as_slice(), [Change::New(_)]), "still new after a change");
    }

    #[test]
    fn numbered_keys_are_not_given_twice() {
        let db = database();
        let (mut a, mut b) = (db.space(), db.space());
        let (one, two) = (add(&mut a, "A", 1.0), add(&mut b, "B", 1.0));
        assert_ne!(one, two);
        a.commit().expect("commits");
        b.commit().expect("commits");
        assert_eq!(names(&a, &Query::all()).len(), 2);
    }

    #[test]
    fn typed_keys_are_needed_and_cannot_repeat() {
        let db = database();
        let mut space = db.space();
        let empty = space.new_object("Bank").expect("the entity");
        assert_eq!(space.save(empty), Err(SpaceError::MissingKey("Bank".into())));
        let mut object = space.new_object("Bank").expect("the entity");
        object.set("code", "001");
        space.save(object.clone()).expect("saves");
        let mut other = db.space();
        other.save(object).expect("saves");
        space.commit().expect("commits");
        assert_eq!(
            other.commit(),
            Err(SpaceError::DuplicateKey { entity: "Bank".into(), key: "001".into() })
        );
        assert!(other.is_modified(), "the changes wait for a decision");
    }

    #[test]
    fn inserting_does_not_take_over_a_record() {
        let db = database();
        let mut space = db.space();
        let bank = |name: &str| {
            let mut object = db.space().new_object("Bank").expect("the entity");
            object.set("code", "001");
            object.set("name", name);
            object
        };
        space.insert(bank("Um")).expect("a new key");
        let waiting = SpaceError::DuplicateKey { entity: "Bank".into(), key: "001".into() };
        assert_eq!(space.insert(bank("Dois")), Err(waiting.clone()), "waiting for the commit");
        space.commit().expect("commits");
        assert_eq!(space.insert(bank("Dois")), Err(waiting), "already kept");
        let key = Value::Text("001".into());
        assert_eq!(
            space.find("Bank", &key).expect("reads").expect("there").get("name"),
            Value::Text("Um".into())
        );
        space.delete("Bank", &key).expect("deletes");
        space.insert(bank("Tres")).expect("free again");
        space.commit().expect("commits");
        assert_eq!(
            space.find("Bank", &key).expect("reads").expect("there").get("name"),
            Value::Text("Tres".into())
        );
    }

    #[test]
    fn a_commit_keeps_all_or_nothing() {
        let db = database();
        let mut first = db.space();
        let key = add(&mut first, "A", 1.0);
        first.commit().expect("commits");
        let mut stale = db.space();
        let mut object = stale.find("Account", &key).expect("reads").expect("there");
        object.set("name", "stale");
        stale.save(object).expect("saves");
        add(&mut stale, "Other", 1.0);
        first.delete("Account", &key).expect("deletes");
        first.commit().expect("commits");
        assert!(matches!(stale.commit(), Err(SpaceError::Conflict(_))));
        assert!(names(&first, &Query::all()).is_empty(), "the new record did not get in either");
    }

    #[test]
    fn filters_sort_and_pages_work_over_calculated_fields() {
        let db = database();
        let mut space = db.space();
        for (name, cents) in [("C", 30.0), ("A", 10.0), ("B", 20.0), ("D", 10.0)] {
            add(&mut space, name, cents);
        }
        space.commit().expect("commits");
        let filter = Criteria::parse("[double] >= 40").expect("parses");
        let sorted = Query::all().filter(filter).sort("cents", false);
        assert_eq!(names(&space, &sorted), ["C", "B"]);
        let by_two = Query::all().sort("cents", true).sort("name", false);
        assert_eq!(names(&space, &by_two), ["D", "A", "B", "C"]);
        assert_eq!(names(&space, &by_two.clone().page(1, 2)), ["A", "B"]);
        let found = space.query("Account", &Query::all().page(0, 1)).expect("reads");
        assert_eq!(found[0].get("double"), Value::Number(60.0), "in key order, computed");
    }

    #[test]
    fn calculated_values_are_not_stored() {
        let db = database();
        let mut space = db.space();
        let key = add(&mut space, "A", 5.0);
        let mut object = space.find("Account", &key).expect("reads").expect("there");
        assert_eq!(object.get("double"), Value::Number(10.0));
        object.set("cents", 7.0);
        space.save(object).expect("saves");
        let found = space.find("Account", &key).expect("reads").expect("there");
        assert_eq!(found.get("double"), Value::Number(14.0));
        let changes = space.changes();
        let [Change::New(pending)] = changes.as_slice() else {
            panic!("one new record");
        };
        assert!(!pending.values().contains_key("double"));
    }

    #[test]
    fn a_new_record_starts_with_its_defaults() {
        let db = database();
        let mut space = db.space();
        let object = space.new_object("Account").expect("the entity");
        assert_eq!(object.get("name"), Value::Text("Nova".into()));
        assert!(!space.is_modified(), "not until it is saved");
    }

    #[test]
    fn a_watcher_can_refuse_a_commit_and_hears_the_rest() {
        let db = database();
        let mut space = db.space();
        let heard = Rc::new(RefCell::new(Vec::new()));
        let log = heard.clone();
        let refuse = Rc::new(std::cell::Cell::new(true));
        let gate = refuse.clone();
        space.on_event(Box::new(move |event| match event {
            SpaceEvent::Committing(_) if gate.get() => Err("not today".to_owned()),
            SpaceEvent::Committing(changes) => {
                log.borrow_mut().push(format!("committing {}", changes.len()));
                Ok(())
            }
            SpaceEvent::Committed(_) => {
                log.borrow_mut().push("committed".to_owned());
                Ok(())
            }
            _ => {
                log.borrow_mut().push("rolled back".to_owned());
                Ok(())
            }
        }));
        add(&mut space, "A", 1.0);
        assert_eq!(space.commit(), Err(SpaceError::Refused("not today".into())));
        assert!(space.is_modified(), "refused, so it still waits");
        refuse.set(false);
        space.commit().expect("commits");
        space.rollback();
        assert_eq!(*heard.borrow(), ["committing 1", "committed", "rolled back"]);
    }

    #[test]
    fn what_does_not_fit_the_entity_says_so() {
        let db = database();
        let mut space = db.space();
        let bad = Query::all().filter(Criteria::parse("[nope] = 1").expect("parses"));
        assert!(matches!(space.query("Account", &bad), Err(SpaceError::BadQuery(_))));
        let bad = Query::all().sort("nope", true);
        assert!(matches!(space.query("Account", &bad), Err(SpaceError::BadQuery(_))));
        assert_eq!(space.new_object("Nope").map(|_| ()), Err(SpaceError::UnknownEntity("Nope".into())));
    }
}
