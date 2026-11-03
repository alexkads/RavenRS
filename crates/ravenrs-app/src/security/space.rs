//! The permissions, applied to every read and every write.

use std::rc::Rc;

use ravenrs_criteria::{Criteria, Value};

use super::{Operation, SecurityContext};
use crate::space::select;
use crate::{
    Change, EntityInfo, FieldInfo, Object, ObjectSpace, Query, SpaceError, SpaceEvent, SpaceWatcher,
};

/// A unit of work that only does what the person signed in can do, whoever asks: a screen, an
/// action or a call with no screen around it.
///
/// A record they cannot read is not there. A field they cannot read reads as empty, also for
/// filters and sorting, so a filter cannot be used to guess it, and writing it back empty leaves
/// what is stored.
pub struct SecuredObjectSpace {
    inner: Box<dyn ObjectSpace>,
    context: Rc<SecurityContext>,
}

impl std::fmt::Debug for SecuredObjectSpace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecuredObjectSpace").field("context", &self.context).finish_non_exhaustive()
    }
}

/// Fields that hold a value, as opposed to listing other records.
fn stored_fields(entity: &EntityInfo) -> impl Iterator<Item = &FieldInfo> {
    entity.fields.iter().filter(|field| !field.kind.is_collection())
}

/// The field a path of an expression starts from.
fn head(path: &str) -> &str {
    path.split('.').next().unwrap_or(path).trim_start_matches('^')
}

/// Whether an expression reaches beyond the fields of the record, as a database could and memory
/// cannot: into a reference, or a collection.
fn reaches_out(criteria: &Criteria) -> bool {
    let mut found = false;
    criteria.walk(&mut |part| match part {
        Criteria::Property(path) if path.len() > 1 => found = true,
        Criteria::Aggregate(_) => found = true,
        _ => {}
    });
    found
}

impl SecuredObjectSpace {
    /// Over this, for the person of this context.
    pub fn new(inner: Box<dyn ObjectSpace>, context: Rc<SecurityContext>) -> Self {
        Self { inner, context }
    }

    fn entity(&self, name: &str) -> Result<EntityInfo, SpaceError> {
        self.context
            .with(|permissions| permissions.entity(name).cloned())
            .ok_or_else(|| SpaceError::UnknownEntity(name.to_owned()))
    }

    /// The record as they can see it; nothing if they cannot see it at all. A field of theirs
    /// keeps its value only if it can be read on this very record.
    fn visible(&self, entity: &EntityInfo, mut record: Object) -> Option<Object> {
        self.context.with(|permissions| {
            let hidden: Vec<&str> = stored_fields(entity)
                .filter(|field| !field.is_key)
                .filter(|field| {
                    !permissions.can_field(Operation::Read, &entity.name, &field.name, Some(&record))
                })
                .map(|field| field.name.as_str())
                .collect();
            let whole = permissions.can(Operation::Read, &entity.name, Some(&record));
            // A permission on a field can open a record its entity keeps closed, but only that.
            let some_field = stored_fields(entity).filter(|field| !field.is_key).count() > hidden.len();
            if !whole && !some_field {
                return None;
            }
            for name in hidden {
                if record.values().contains_key(name) {
                    record.set(name, Value::Null);
                }
            }
            Some(record)
        })
    }

    /// Whether a query can go to storage as it is, because no record and no field it mentions
    /// depends on anything storage does not know.
    fn storage_can_answer(&self, entity: &EntityInfo, query: &Query) -> bool {
        self.context.with(|permissions| {
            if permissions.depends_on_record(&entity.name)
                || !permissions.can(Operation::Read, &entity.name, None)
            {
                return false;
            }
            let readable = |name: &str| {
                entity.field_named(name).is_some_and(|field| !field.secret)
                    && permissions.can_field(Operation::Read, &entity.name, name, None)
            };
            let filter_fits = query.filter.as_ref().is_none_or(|filter| {
                !reaches_out(filter) && filter.properties().iter().all(|path| readable(head(path)))
            });
            filter_fits && query.sort.iter().all(|(field, _)| readable(field))
        })
    }

    fn check_new(&self, entity: &EntityInfo, object: &Object) -> Result<(), SpaceError> {
        let fresh = entity.new_object();
        self.context.with(|permissions| {
            let name = &entity.name;
            let mut object = object.clone();
            entity.compute(&mut object, permissions.environment());
            let object = &object;
            if !permissions.can(Operation::Create, name, Some(object))
                || !permissions.can(Operation::Write, name, Some(object))
            {
                return Err(self.context.denied());
            }
            for field in stored_fields(entity).filter(|field| !field.is_key && field.calculated.is_none()) {
                let value = object.get(&field.name);
                if value != fresh.get(&field.name)
                    && !permissions.can_field(Operation::Write, name, &field.name, Some(object))
                {
                    return Err(self.context.denied());
                }
            }
            Ok(())
        })
    }

    /// What is written over what is stored: every field changed has to be theirs to write, before
    /// and after, and a field they cannot read written back empty stays as it was. Nothing changed
    /// means nothing to write, as writing it would bring back the whole record as it stood.
    fn over_stored(
        &self,
        entity: &EntityInfo,
        mut object: Object,
        stored: &Object,
    ) -> Result<Option<Object>, SpaceError> {
        self.context.with(|permissions| {
            let name = &entity.name;
            let mut changed = false;
            let writable = || stored_fields(entity).filter(|field| field.calculated.is_none());
            for field in writable() {
                let (new, old) = (object.get(&field.name), stored.get(&field.name));
                if new == old {
                    // Sending the very value of a field they cannot see is guessing it: if that
                    // went through and any other value did not, they would learn what it holds.
                    if new != Value::Null
                        && !permissions.can_field(Operation::Read, name, &field.name, Some(stored))
                        && !permissions.can_field(Operation::Write, name, &field.name, Some(stored))
                    {
                        return Err(self.context.denied());
                    }
                    continue;
                }
                if new == Value::Null
                    && !permissions.can_field(Operation::Read, name, &field.name, Some(stored))
                {
                    object.set(&field.name, old);
                    continue;
                }
                if !permissions.can_field(Operation::Write, name, &field.name, Some(stored)) {
                    return Err(self.context.denied());
                }
                changed = true;
            }
            if !changed {
                return Ok(None);
            }
            // What the rules read after the change is worked out here, and not taken from the caller.
            let mut after = object.clone();
            entity.compute(&mut after, permissions.environment());
            for field in writable() {
                if after.get(&field.name) != stored.get(&field.name)
                    && !permissions.can_field(Operation::Write, name, &field.name, Some(&after))
                {
                    return Err(self.context.denied());
                }
            }
            Ok(Some(object))
        })
    }
}

/// A waiting change as the person can see it: what they cannot read goes empty, as a save fills it
/// in from storage for the commit.
fn as_seen(context: &SecurityContext, change: &Change) -> Change {
    let masked = |object: &Object| {
        context.with(|permissions| {
            let Some(entity) = permissions.entity(object.entity()) else { return object.clone() };
            let mut seen = object.clone();
            for field in stored_fields(entity).filter(|field| !field.is_key) {
                if !permissions.can_field(Operation::Read, &entity.name, &field.name, Some(object)) {
                    seen.set(&field.name, Value::Null);
                }
            }
            seen
        })
    };
    match change {
        Change::New(object) => Change::New(masked(object)),
        Change::Updated(object) => Change::Updated(masked(object)),
        Change::Deleted { entity, key } => Change::Deleted { entity: entity.clone(), key: key.clone() },
    }
}

impl ObjectSpace for SecuredObjectSpace {
    fn entities(&self) -> Vec<EntityInfo> {
        self.inner.entities()
    }

    fn query(&self, entity: &str, query: &Query) -> Result<Vec<Object>, SpaceError> {
        let info = self.entity(entity)?;
        if self.storage_can_answer(&info, query) {
            let found = self.inner.query(entity, query)?;
            return Ok(found.into_iter().filter_map(|record| self.visible(&info, record)).collect());
        }
        crate::space::check_query(&info, query)?;
        // Everything is read, and only what they can see gets filtered, sorted and paged, so the
        // size of a page and the order say nothing about the rest. Everything whole, since a
        // permission can depend on a field the list does not show.
        let everything = self.inner.query(entity, &Query::all())?;
        let seen: Vec<Object> =
            everything.into_iter().filter_map(|record| self.visible(&info, record)).collect();
        self.context.with(|permissions| select(&info, seen, query, permissions.environment()))
    }

    fn find(&self, entity: &str, key: &Value) -> Result<Option<Object>, SpaceError> {
        let info = self.entity(entity)?;
        Ok(self.inner.find(entity, key)?.and_then(|record| self.visible(&info, record)))
    }

    fn new_object(&mut self, entity: &str) -> Result<Object, SpaceError> {
        let allowed = self.context.with(|permissions| permissions.can(Operation::Create, entity, None));
        if !allowed {
            return Err(self.context.denied());
        }
        self.inner.new_object(entity)
    }

    fn save(&mut self, object: Object) -> Result<(), SpaceError> {
        let info = self.entity(object.entity())?;
        let key = info.key_field().map(|field| object.get(&field.name)).unwrap_or(Value::Null);
        match self.inner.find(&info.name, &key)? {
            Some(stored) => match self.over_stored(&info, object, &stored)? {
                Some(written) => self.inner.save(written),
                None => Ok(()),
            },
            None => {
                self.check_new(&info, &object)?;
                self.inner.save(object)
            }
        }
    }

    fn insert(&mut self, object: Object) -> Result<(), SpaceError> {
        let info = self.entity(object.entity())?;
        self.check_new(&info, &object)?;
        self.inner.insert(object)
    }

    fn delete(&mut self, entity: &str, key: &Value) -> Result<(), SpaceError> {
        let Some(stored) = self.inner.find(entity, key)? else { return self.inner.delete(entity, key) };
        let allowed =
            self.context.with(|permissions| permissions.can(Operation::Delete, entity, Some(&stored)));
        if !allowed {
            return Err(self.context.denied());
        }
        self.inner.delete(entity, key)
    }

    fn changes(&self) -> Vec<Change> {
        let context = &self.context;
        self.inner.changes().iter().map(|change| as_seen(context, change)).collect()
    }

    fn is_modified(&self) -> bool {
        self.inner.is_modified()
    }

    fn commit(&mut self) -> Result<(), SpaceError> {
        let touched = self.inner.changes().iter().any(|change| match change {
            Change::New(object) | Change::Updated(object) => self.context.touches_security(object.entity()),
            Change::Deleted { entity, .. } => self.context.touches_security(entity),
        });
        // What is waiting reads as if it were kept, so this sees the result of the commit.
        if touched && !self.context.directory().has_active_administrator(&*self.inner)? {
            return Err(SpaceError::Refused(
                "At least one active user has to keep an administrative role.".to_owned(),
            ));
        }
        self.inner.commit()?;
        if touched {
            self.context.reload()?;
        }
        Ok(())
    }

    fn rollback(&mut self) {
        self.inner.rollback();
    }

    fn refresh(&mut self) {
        self.inner.refresh();
    }

    fn on_event(&mut self, mut watcher: SpaceWatcher) {
        // Whoever watches sees what the person can see, and not what is going to storage.
        let context = self.context.clone();
        self.inner.on_event(Box::new(move |event| match event {
            SpaceEvent::Committing(changes) => {
                let seen: Vec<Change> = changes.iter().map(|change| as_seen(&context, change)).collect();
                watcher(SpaceEvent::Committing(&seen))
            }
            SpaceEvent::Committed(changes) => {
                let seen: Vec<Change> = changes.iter().map(|change| as_seen(&context, change)).collect();
                watcher(SpaceEvent::Committed(&seen))
            }
            other => watcher(other),
        }));
    }
}
