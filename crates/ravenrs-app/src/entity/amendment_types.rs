//! Sealed records and the amendments that are the only way to change them.
//!
//! A sealed entity says by an expression when one of its records, as stored, is closed. Such a
//! record is not changed or deleted directly: only an amendment, a record of another entity that
//! points to it with a reason, changes it, and in the same commit. The amendment is kept as it was
//! written and never changes.

use std::collections::BTreeSet;
use std::fmt;
use std::rc::Rc;

use ravenrs_criteria::{Criteria, Scope, Value};

use crate::space::key_text;
use crate::{Change, EntityInfo, FieldKind, ModelError, Object, ObjectSpace, SpaceError};

/// When a record of an entity is closed, by what storage has of it.
///
/// ```
/// use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, Seal};
///
/// let lesson = EntityInfo::new("Lesson")
///     .field(FieldInfo::new("id", FieldKind::Integer).key())
///     .field(FieldInfo::new("status", FieldKind::Text))
///     .sealed(Seal::when("[status] = 'closed'"));
/// assert!(lesson.check().is_ok());
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Seal {
    /// The expression a stored record meets when it is closed.
    pub when: String,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Seal {
    /// Closed when the stored record meets this expression.
    pub fn when(expression: impl Into<String>) -> Self {
        Self { when: expression.into() }
    }

    /// Whether this record, as it is, meets the seal. One that does not read never seals.
    pub fn holds(&self, entity: &EntityInfo, object: &Object) -> bool {
        let environment = crate::environment();
        let mut object = object.clone();
        entity.compute(&mut object, &environment);
        Criteria::parse(&self.when)
            .ok()
            .is_some_and(|criteria| Scope::new(&environment).row(&object).matches(&criteria))
    }
}

/// What an amendment does to the record it points to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
pub enum AmendmentKind {
    /// It corrects a closed record, which stays closed.
    Correction,
    /// It closes, ends or transfers a record still open, which is closed after it. A record has
    /// one at most.
    Closing,
}

/// What an amendment does to its target besides copying fields, in the same commit: given the
/// amendment, the target as it is about to be kept, and the unit of work.
pub type AmendmentApply = Rc<dyn Fn(&Object, &mut Object, &mut dyn ObjectSpace) -> Result<(), String>>;

/// An entity whose records amend the closed records of another.
///
/// ```
/// use ravenrs_app::{Amendment, EntityInfo, FieldInfo, FieldKind, Seal};
///
/// let correction = EntityInfo::new("LessonCorrection")
///     .field(FieldInfo::new("id", FieldKind::Integer).key())
///     .field(FieldInfo::new("lesson_id", FieldKind::Reference("Lesson".into())))
///     .field(FieldInfo::new("present", FieldKind::Integer))
///     .field(FieldInfo::new("reason", FieldKind::LongText))
///     .field(FieldInfo::new("previous", FieldKind::LongText))
///     .amends(
///         Amendment::correction("Lesson", "lesson_id")
///             .copies(["present"])
///             .reason("reason", 10)
///             .before("previous"),
///     );
/// assert!(correction.check().is_ok());
/// ```
#[derive(Clone)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Amendment {
    /// What it does to its target.
    pub kind: AmendmentKind,
    /// The entity it amends.
    pub target: String,
    /// Its field that holds the key of the record it amends.
    pub reference: String,
    /// Fields of the same name it writes on the target.
    pub copies: Vec<String>,
    /// Its field with the reason, and how many characters the reason needs at least.
    pub reason: Option<(String, usize)>,
    /// Its field where the framework writes, as JSON, the target's values that change.
    pub before: Option<String>,
    /// A yes-or-no field of the target the framework sets once it is amended.
    pub marks: Option<String>,
    apply: Option<AmendmentApply>,
}

impl fmt::Debug for Amendment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Amendment")
            .field("kind", &self.kind)
            .field("target", &self.target)
            .field("reference", &self.reference)
            .field("copies", &self.copies)
            .field("reason", &self.reason)
            .field("before", &self.before)
            .field("marks", &self.marks)
            .field("apply", &self.apply.is_some())
            .finish()
    }
}

impl PartialEq for Amendment {
    fn eq(&self, other: &Self) -> bool {
        let same_apply = match (&self.apply, &other.apply) {
            (Some(a), Some(b)) => Rc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        self.kind == other.kind
            && self.target == other.target
            && self.reference == other.reference
            && self.copies == other.copies
            && self.reason == other.reason
            && self.before == other.before
            && self.marks == other.marks
            && same_apply
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Amendment {
    fn of(kind: AmendmentKind, target: impl Into<String>, reference: impl Into<String>) -> Self {
        Self {
            kind,
            target: target.into(),
            reference: reference.into(),
            copies: Vec::new(),
            reason: None,
            before: None,
            marks: None,
            apply: None,
        }
    }

    /// Corrects a closed record of `target`, the one whose key is in `reference`.
    pub fn correction(target: impl Into<String>, reference: impl Into<String>) -> Self {
        Self::of(AmendmentKind::Correction, target, reference)
    }

    /// Closes, ends or transfers a record of `target` still open, the one whose key is in
    /// `reference`. What it writes has to leave the target closed.
    pub fn closing(target: impl Into<String>, reference: impl Into<String>) -> Self {
        Self::of(AmendmentKind::Closing, target, reference)
    }

    /// Writes these fields, which have the same name on both, on the target.
    pub fn copies<I, S>(mut self, fields: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.copies.extend(fields.into_iter().map(Into::into));
        self
    }

    /// Asks for a reason in this text field, of at least this many characters.
    pub fn reason(mut self, field: impl Into<String>, at_least: usize) -> Self {
        self.reason = Some((field.into(), at_least));
        self
    }

    /// Keeps in this text field the target's values that change, as they were.
    pub fn before(mut self, field: impl Into<String>) -> Self {
        self.before = Some(field.into());
        self
    }

    /// Sets this yes-or-no field of the target.
    pub fn marks(mut self, field: impl Into<String>) -> Self {
        self.marks = Some(field.into());
        self
    }

    /// Does more to the target, or to other records, in the same commit: a transfer creates the
    /// record it goes to, a correction can bring a state back. An error stops it all.
    pub fn apply(
        mut self,
        apply: impl Fn(&Object, &mut Object, &mut dyn ObjectSpace) -> Result<(), String> + 'static,
    ) -> Self {
        self.apply = Some(Rc::new(apply));
        self
    }

    /// What is wrong with it on this entity, which holds it.
    pub(crate) fn problem(&self, entity: &EntityInfo) -> Option<String> {
        let text = |kind: &FieldKind| matches!(kind, FieldKind::Text | FieldKind::LongText);
        match entity.field_named(&self.reference).map(|field| &field.kind) {
            Some(FieldKind::Reference(target)) if *target == self.target => {}
            Some(_) => return Some(format!("{} does not refer to {}", self.reference, self.target)),
            None => return Some(format!("it amends through {}, which is not a field", self.reference)),
        }
        if let Some((field, _)) = &self.reason
            && !entity.field_named(field).is_some_and(|field| text(&field.kind))
        {
            return Some(format!("the reason {field} is not a text field"));
        }
        if let Some(field) = &self.before
            && !entity.field_named(field).is_some_and(|field| text(&field.kind))
        {
            return Some(format!("{field}, which keeps what was before, is not a text field"));
        }
        self.copies
            .iter()
            .find(|field| entity.field_named(field).is_none())
            .map(|field| format!("it copies {field}, which is not a field"))
    }
}

/// Whether amendments fit across the entities: each target is there and sealed, and the fields an
/// amendment copies or marks are on its target.
pub fn check_amendment_model(entities: &[EntityInfo]) -> Result<(), ModelError> {
    for entity in entities {
        let Some(amendment) = &entity.amendment else { continue };
        let wrong = |what: String| Err(ModelError(format!("{}: {what}", entity.name)));
        let Some(target) = entities.iter().find(|known| known.name == amendment.target) else {
            return wrong(format!("it amends {}, which is not an entity", amendment.target));
        };
        if target.seal.is_none() {
            return wrong(format!("it amends {}, which is not sealed", target.name));
        }
        if let Some(field) = amendment.copies.iter().find(|field| target.field_named(field).is_none()) {
            return wrong(format!("it copies {field}, which {} does not have", target.name));
        }
        if let Some(field) = &amendment.marks
            && !target.field_named(field).is_some_and(|field| matches!(field.kind, FieldKind::Boolean))
        {
            return wrong(format!("it marks {field}, which is not a yes-or-no field of {}", target.name));
        }
    }
    Ok(())
}

fn refused(entity: &EntityInfo, why: impl fmt::Display) -> SpaceError {
    SpaceError::Refused(format!("{}: {why}", entity.caption))
}

fn entity_of<'a>(entities: &'a [EntityInfo], name: &str) -> Result<&'a EntityInfo, SpaceError> {
    entities.iter().find(|entity| entity.name == name).ok_or_else(|| SpaceError::UnknownEntity(name.into()))
}

/// The key value of a record.
fn key_value(entity: &EntityInfo, object: &Object) -> Value {
    entity.key_field().map_or(Value::Null, |field| object.get(&field.name))
}

/// What people use to change a closed record of this entity.
fn way_to_amend(entities: &[EntityInfo], target: &str) -> String {
    entities
        .iter()
        .find(|entity| {
            entity.amendment.as_ref().is_some_and(|amendment| {
                amendment.target == target && amendment.kind == AmendmentKind::Correction
            })
        })
        .map_or_else(|| "an amendment".to_owned(), |entity| entity.caption.clone())
}

/// Reads a stored record, as storage has it without what waits.
pub type StoredLookup<'a> = dyn FnMut(&EntityInfo, &Value) -> Result<Option<Object>, SpaceError> + 'a;

/// Whether these changes keep to the seals and amendments of the entities. Whoever keeps records
/// calls this with everything about to be committed and a way to read what storage has now,
/// before writing, as every space here does.
///
/// A sealed record, closed as stored, is changed or deleted only by an amendment that points to it
/// in the same changes. An amendment needs its reason; a correction needs a closed target, a
/// closing an open one, which it leaves closed. An amendment kept is never changed or deleted.
pub fn check_amendments(
    entities: &[EntityInfo],
    changes: &[Change],
    stored: &mut StoredLookup<'_>,
) -> Result<(), SpaceError> {
    let mut amended: BTreeSet<(String, String)> = BTreeSet::new();
    for change in changes {
        let Change::New(object) = change else { continue };
        let entity = entity_of(entities, object.entity())?;
        let Some(amendment) = &entity.amendment else { continue };
        let target = entity_of(entities, &amendment.target)?;
        let key = object.get(&amendment.reference);
        let Some(text) = key_text(&key) else {
            return Err(refused(entity, format!("it says no {}", target.caption)));
        };
        if let Some((field, at_least)) = &amendment.reason
            && object.text(field).trim().chars().count() < *at_least
        {
            return Err(refused(entity, format!("the reason needs at least {at_least} characters")));
        }
        let Some(kept) = stored(target, &key)? else {
            return Err(refused(entity, format!("there is no {} {text}", target.caption)));
        };
        let closed = target.seal.as_ref().is_some_and(|seal| seal.holds(target, &kept));
        match amendment.kind {
            AmendmentKind::Correction if !closed => {
                return Err(refused(target, "it is not closed: correct it on its own form"));
            }
            AmendmentKind::Closing if closed => return Err(refused(target, "it is closed already")),
            AmendmentKind::Closing => {
                let after = changes.iter().find_map(|change| match change {
                    Change::Updated(object)
                        if object.entity() == target.name
                            && key_text(&key_value(target, object)).as_deref() == Some(&text) =>
                    {
                        Some(object)
                    }
                    _ => None,
                });
                let closes = after
                    .is_some_and(|after| target.seal.as_ref().is_some_and(|seal| seal.holds(target, after)));
                if !closes {
                    return Err(refused(entity, format!("it has to leave the {} closed", target.caption)));
                }
            }
            AmendmentKind::Correction => {}
        }
        if !amended.insert((target.name.clone(), text)) && amendment.kind == AmendmentKind::Closing {
            return Err(refused(target, "it can be closed only once"));
        }
    }
    for change in changes {
        let (name, key) = match change {
            Change::Updated(object) => {
                (object.entity().to_owned(), key_value(entity_of(entities, object.entity())?, object))
            }
            Change::Deleted { entity, key } => (entity.clone(), key.clone()),
            Change::New(_) => continue,
        };
        let entity = entity_of(entities, &name)?;
        if entity.amendment.is_some() {
            if stored(entity, &key)?.is_some() {
                return Err(refused(entity, "an amendment that is kept does not change"));
            }
            continue;
        }
        let Some(seal) = &entity.seal else { continue };
        let Some(text) = key_text(&key) else { continue };
        if amended.contains(&(name.clone(), text)) {
            continue;
        }
        if stored(entity, &key)?.is_some_and(|kept| seal.holds(entity, &kept)) {
            return Err(refused(entity, format!("it is closed: use {}", way_to_amend(entities, &name))));
        }
    }
    Ok(())
}

/// Writes an amendment on its target, in the same unit of work: reads the target as storage has
/// it, keeps in the amendment what is about to change, copies and marks, and runs what the
/// amendment applies. Gives the amendment as it is to be kept. Whoever keeps records calls this as
/// a new amendment comes in, so whatever wraps the space sees the change to the target; a space
/// that fails here throws away all this did.
pub fn apply_amendment(
    space: &mut dyn ObjectSpace,
    entity: &EntityInfo,
    mut object: Object,
    stored: &mut StoredLookup<'_>,
    first_time: bool,
) -> Result<Object, SpaceError> {
    let Some(amendment) = entity.amendment.clone() else { return Ok(object) };
    let entities = space.entities();
    let target = entity_of(&entities, &amendment.target)?.clone();
    let key = object.get(&amendment.reference);
    // A missing target is refused by the checks of the commit, with what to say.
    let Some(kept) = stored(&target, &key)? else { return Ok(object) };
    let mut after = space.find(&target.name, &key)?.unwrap_or_else(|| kept.clone());
    for field in &amendment.copies {
        after.set(field, object.get(field));
    }
    if let Some(field) = &amendment.marks {
        after.set(field, Value::Bool(true));
    }
    if first_time && let Some(apply) = &amendment.apply {
        apply(&object, &mut after, space).map_err(|why| refused(entity, why))?;
    }
    if let Some(field) = &amendment.before {
        let was: serde_json::Map<String, serde_json::Value> = target
            .fields
            .iter()
            .filter(|field| field.calculated.is_none() && !matches!(field.kind, FieldKind::Collection(_)))
            .filter(|field| kept.get(&field.name) != after.get(&field.name))
            .map(|field| (field.name.clone(), crate::space::value_to_json(&kept.get(&field.name))))
            .collect();
        object.set(field, Value::Text(serde_json::Value::Object(was).to_string()));
    }
    space.save(after)?;
    Ok(object)
}
