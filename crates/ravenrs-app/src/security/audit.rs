//! A record of who changed what, written in the same commit as the change.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use ravenrs_criteria::Value;

use super::{SECURITY_AUDIT_ENTRY, SpaceMaker};
use crate::space::key_text;
use crate::{Change, EntityInfo, FieldInfo, FieldKind, Object, ObjectSpace, Query, SpaceError, SpaceWatcher};

/// How much is written about the records of an entity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum AuditMode {
    /// Everything, with the fields and their values, before and after.
    #[default]
    Full,
    /// That a record was created, changed or deleted, with no fields or values.
    Lightweight,
    /// Only that a record was created, as when bringing in a lot of records.
    CreationOnly,
    /// Nothing.
    Off,
}

/// What the audit trail covers.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct AuditSettings {
    mode: AuditMode,
    modes: BTreeMap<String, AuditMode>,
    left_out: BTreeSet<(String, String)>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl AuditSettings {
    /// Everything, in this mode.
    pub fn new(mode: AuditMode) -> Self {
        Self { mode, ..Self::default() }
    }

    /// This entity in another mode.
    pub fn entity(mut self, entity: impl Into<String>, mode: AuditMode) -> Self {
        self.modes.insert(entity.into(), mode);
        self
    }

    /// Leaves this field out.
    pub fn without_field(mut self, entity: impl Into<String>, field: impl Into<String>) -> Self {
        self.left_out.insert((entity.into(), field.into()));
        self
    }

    fn mode_of(&self, entity: &str) -> AuditMode {
        if entity == SECURITY_AUDIT_ENTRY {
            return AuditMode::Off;
        }
        self.modes.get(entity).copied().unwrap_or(self.mode)
    }

    fn covers(&self, field: &FieldInfo, entity: &str) -> bool {
        !field.kind.is_collection()
            && field.calculated.is_none()
            && !self.left_out.contains(&(entity.to_owned(), field.name.clone()))
    }
}

/// One line of the trail, before it is written.
#[derive(Clone, Debug)]
struct Line {
    change: &'static str,
    entity: String,
    key: String,
    field: String,
    old: Value,
    new: Value,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Line {
    fn of(change: &'static str, entity: &str, key: &str) -> Self {
        Self {
            change,
            entity: entity.to_owned(),
            key: key.to_owned(),
            field: String::new(),
            old: Value::Null,
            new: Value::Null,
        }
    }

    fn field(mut self, name: &str) -> Self {
        self.field = name.to_owned();
        self
    }

    fn values(mut self, old: Value, new: Value) -> Self {
        self.old = old;
        self.new = new;
        self
    }
}

/// What the audit trail needs: what it covers, who is acting, and what to add to the next commit.
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct AuditTrail {
    settings: AuditSettings,
    author: Rc<dyn Fn() -> String>,
    waiting: RefCell<Vec<Line>>,
    /// Writes what was noted outside a commit, as nothing else is on the way for it.
    writer: RefCell<Option<SpaceMaker>>,
}

impl std::fmt::Debug for AuditTrail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuditTrail").field("settings", &self.settings).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl AuditTrail {
    /// With these settings; `author` says who is acting.
    pub fn new(settings: AuditSettings, author: impl Fn() -> String + 'static) -> Self {
        Self { settings, author: Rc::new(author), waiting: RefCell::default(), writer: RefCell::default() }
    }

    /// Writes notes there right away, instead of with the next commit.
    pub fn write_with(&self, space: SpaceMaker) {
        *self.writer.borrow_mut() = Some(space);
    }

    /// Adds something of the application's to the trail, about this record and field. It is
    /// written right away with [`write_with`](Self::write_with), or else with the next commit.
    pub fn custom(&self, entity: &str, key: &Value, field: &str, what: &str) {
        let line = Line::of("custom", entity, &key_text(key).unwrap_or_default())
            .field(field)
            .values(Value::Null, Value::Text(what.to_owned()));
        let writer = self.writer.borrow().clone();
        if let Some(writer) = writer {
            let mut space = writer();
            if self.write(&mut *space, std::slice::from_ref(&line)).is_ok() && space.commit().is_ok() {
                return;
            }
        }
        self.waiting.borrow_mut().push(line);
    }

    /// Adds these lines to what waits in this space, and says what it added.
    fn write(&self, space: &mut dyn ObjectSpace, lines: &[Line]) -> Result<Vec<Value>, SpaceError> {
        let (author, now) = ((self.author)(), crate::local_now());
        let mut added = Vec::with_capacity(lines.len());
        for line in lines {
            let mut entry = space.new_object(SECURITY_AUDIT_ENTRY)?;
            let key = entry.get("id");
            entry.set("happened_at", Value::Date(now));
            entry.set("author", author.clone());
            entry.set("change", line.change);
            entry.set("target_entity", line.entity.clone());
            entry.set("target_key", line.key.clone());
            entry.set("field", line.field.clone());
            entry.set("old_value", line.old.clone());
            entry.set("new_value", line.new.clone());
            space.insert(entry)?;
            added.push(key);
        }
        Ok(added)
    }
}

/// A value as the trail shows it. What is too big to read is only said to be there.
fn shown(field: &FieldInfo, value: &Value) -> Value {
    if field.secret || matches!(field.kind, FieldKind::Image | FieldKind::File) {
        return Value::Null;
    }
    match value {
        Value::Null => Value::Null,
        Value::Text(text) => Value::Text(text.clone()),
        Value::Number(number) => Value::Text(number.to_string()),
        Value::Bool(on) => Value::Text(on.to_string()),
        Value::Date(date) => {
            let (year, month, day) = date.ymd();
            let (hour, minute) = (date.seconds / 3600, date.seconds % 3600 / 60);
            Value::Text(format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}"))
        }
        _ => Value::Null,
    }
}

/// A unit of work that writes what it commits to the audit trail, in the same commit, so a change
/// and its trail stay or go together. Changes made to the database some other way are not seen.
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct AuditedObjectSpace {
    inner: Box<dyn ObjectSpace>,
    trail: Rc<AuditTrail>,
    entities: Vec<EntityInfo>,
    /// Each record as it was before this unit of work first touched it.
    before: BTreeMap<(String, String), Option<Object>>,
}

impl std::fmt::Debug for AuditedObjectSpace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuditedObjectSpace").field("trail", &self.trail).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl AuditedObjectSpace {
    /// Over this, writing to this trail.
    pub fn new(inner: Box<dyn ObjectSpace>, trail: Rc<AuditTrail>) -> Self {
        let entities = inner.entities();
        Self { inner, trail, entities, before: BTreeMap::new() }
    }

    fn entity(&self, name: &str) -> Option<&EntityInfo> {
        self.entities.iter().find(|entity| entity.name == name)
    }

    fn remember(&mut self, entity: &str, key: &Value) -> Result<(), SpaceError> {
        let Some(text) = key_text(key) else { return Ok(()) };
        let slot = (entity.to_owned(), text);
        if !self.before.contains_key(&slot) {
            let found = self.inner.find(entity, key)?;
            self.before.insert(slot, found);
        }
        Ok(())
    }

    fn remember_object(&mut self, object: &Object) -> Result<(), SpaceError> {
        let key = self
            .entity(object.entity())
            .and_then(|entity| entity.key_field())
            .map(|field| object.get(&field.name));
        match key {
            Some(key) => self.remember(object.entity(), &key),
            None => Ok(()),
        }
    }

    /// The collections that list records of this entity through this reference.
    fn collections_of(&self, entity: &str, reference: &FieldInfo) -> Vec<(&EntityInfo, &FieldInfo)> {
        let FieldKind::Reference(owner) = &reference.kind else { return Vec::new() };
        let Some(owner) = self.entity(owner) else { return Vec::new() };
        owner
            .fields
            .iter()
            .filter(|field| matches!(&field.kind, FieldKind::Collection(target) if target == entity))
            .filter(|field| field.back_reference.as_ref().is_none_or(|back| *back == reference.name))
            .map(|field| (owner, field))
            .collect()
    }

    /// What this change says about the lists that hold the record, for the records that own them.
    fn collection_lines(
        &self,
        entity: &EntityInfo,
        key: &str,
        old: Option<&Object>,
        new: Option<&Object>,
        lines: &mut Vec<Line>,
    ) {
        let settings = &self.trail.settings;
        for reference in entity.fields.iter().filter(|field| matches!(field.kind, FieldKind::Reference(_))) {
            let was = old.map_or(Value::Null, |object| object.get(&reference.name));
            let is = new.map_or(Value::Null, |object| object.get(&reference.name));
            for (owner, collection) in self.collections_of(&entity.name, reference) {
                if settings.mode_of(&owner.name) != AuditMode::Full {
                    continue;
                }
                let mut note = |change, on: &Value| {
                    if let Some(text) = key_text(on) {
                        lines.push(
                            Line::of(change, &owner.name, &text)
                                .field(&collection.name)
                                .values(Value::Null, Value::Text(format!("{} {key}", entity.name))),
                        );
                    }
                };
                if was != is {
                    note("removed_from_collection", &was);
                    note("added_to_collection", &is);
                } else if let (Some(old), Some(new)) = (old, new)
                    && entity
                        .fields
                        .iter()
                        .any(|f| settings.covers(f, &entity.name) && old.get(&f.name) != new.get(&f.name))
                {
                    note("collection_object_changed", &is);
                }
            }
        }
    }

    fn lines_of(&self, changes: &[Change]) -> Vec<Line> {
        let settings = &self.trail.settings;
        let mut lines = Vec::new();
        for change in changes {
            let (name, key_value, object) = match change {
                Change::New(object) | Change::Updated(object) => {
                    let key = self
                        .entity(object.entity())
                        .and_then(EntityInfo::key_field)
                        .map_or(Value::Null, |field| object.get(&field.name));
                    (object.entity().to_owned(), key, Some(object))
                }
                Change::Deleted { entity, key } => (entity.clone(), key.clone(), None),
            };
            let Some(entity) = self.entity(&name) else { continue };
            let mode = settings.mode_of(&name);
            let key = key_text(&key_value).unwrap_or_default();
            let old = self
                .before
                .get(&(name.clone(), key.clone()))
                .and_then(Option::as_ref)
                .filter(|_| !matches!(change, Change::New(_)));
            match (mode, change) {
                (AuditMode::Off, _) => {}
                (_, Change::New(_)) => lines.push(Line::of("created", &name, &key)),
                (AuditMode::CreationOnly, _) => {}
                (AuditMode::Lightweight, Change::Updated(_)) => lines.push(Line::of("changed", &name, &key)),
                (_, Change::Deleted { .. }) => lines.push(Line::of("deleted", &name, &key)),
                _ => {}
            }
            if mode != AuditMode::Full {
                continue;
            }
            if let Some(object) = object {
                let fresh = entity.new_object();
                for field in entity.fields.iter().filter(|field| settings.covers(field, &name)) {
                    let new = object.get(&field.name);
                    match (change, old) {
                        (Change::New(_), _) => {
                            if !field.is_key && new != fresh.get(&field.name) {
                                lines.push(
                                    Line::of("initial_value", &name, &key)
                                        .field(&field.name)
                                        .values(Value::Null, shown(field, &new)),
                                );
                            }
                        }
                        (_, Some(old)) if old.get(&field.name) != new => lines.push(
                            Line::of("changed", &name, &key)
                                .field(&field.name)
                                .values(shown(field, &old.get(&field.name)), shown(field, &new)),
                        ),
                        _ => {}
                    }
                }
            }
            self.collection_lines(entity, &key, old, object, &mut lines);
        }
        lines
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose(part = "objectspace"))]
impl ObjectSpace for AuditedObjectSpace {
    fn entities(&self) -> Vec<EntityInfo> {
        self.inner.entities()
    }

    fn query(&self, entity: &str, query: &Query) -> Result<Vec<Object>, SpaceError> {
        self.inner.query(entity, query)
    }

    fn find(&self, entity: &str, key: &Value) -> Result<Option<Object>, SpaceError> {
        self.inner.find(entity, key)
    }

    fn new_object(&mut self, entity: &str) -> Result<Object, SpaceError> {
        self.inner.new_object(entity)
    }

    fn save(&mut self, object: Object) -> Result<(), SpaceError> {
        self.remember_object(&object)?;
        self.inner.save(object)
    }

    fn insert(&mut self, object: Object) -> Result<(), SpaceError> {
        self.remember_object(&object)?;
        self.inner.insert(object)
    }

    fn delete(&mut self, entity: &str, key: &Value) -> Result<(), SpaceError> {
        self.remember(entity, key)?;
        self.inner.delete(entity, key)
    }

    fn changes(&self) -> Vec<Change> {
        self.inner.changes()
    }

    fn commit(&mut self) -> Result<(), SpaceError> {
        let changes = self.inner.changes();
        let mut lines = self.lines_of(&changes);
        let noted: Vec<Line> = self.trail.waiting.borrow().clone();
        lines.extend(noted.iter().cloned());
        let written = self.trail.write(&mut *self.inner, &lines)?;
        match self.inner.commit() {
            Ok(()) => {
                self.trail.waiting.borrow_mut().drain(..noted.len());
                self.before.clear();
                Ok(())
            }
            Err(error) => {
                // What was added goes away, so trying again does not write it twice.
                for key in written {
                    self.inner.delete(SECURITY_AUDIT_ENTRY, &key)?;
                }
                Err(error)
            }
        }
    }

    fn rollback(&mut self) {
        self.before.clear();
        self.inner.rollback();
    }

    fn refresh(&mut self) {
        self.before.clear();
        self.inner.refresh();
    }

    fn on_event(&mut self, watcher: SpaceWatcher) {
        self.inner.on_event(watcher);
    }
}
