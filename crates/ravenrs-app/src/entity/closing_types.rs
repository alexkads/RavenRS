//! Periods that are approved and closed, with numbered versions and a justified reopening.
//!
//! A period entity, such as a month, a term or a payroll, declares which records belong to it and
//! when it may be approved. Approving keeps the final document with its hash in a numbered version
//! and freezes every record of the period: none is added, changed or deleted until the version is
//! reopened with a reason. A version, once kept, changes only from current to reopened.
//!
//! The rules run where records are kept, at the commit, as the seals and amendments do: any
//! space that keeps records holds to them, whoever writes.

use std::collections::BTreeSet;
use std::fmt;

use ravenrs_criteria::{Criteria, Value};
use sha2::{Digest, Sha256};

use crate::space::key_text;
use crate::{
    AttachmentStore, Change, EntityInfo, FieldInfo, FieldKind, ModelError, Object, ObjectSpace, Query,
    SpaceError, StoredLookup,
};

/// The status of a version that holds its period closed.
pub const CLOSING_CURRENT: &str = "current";
/// The status of a version whose period was opened again.
pub const CLOSING_REOPENED: &str = "reopened";

/// How a period entity is approved and closed.
///
/// ```
/// use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, PeriodClosing};
///
/// let term = EntityInfo::new("Term")
///     .field(FieldInfo::new("id", FieldKind::Integer).key())
///     .period_closing(
///         PeriodClosing::new("TermClosing")
///             .freezes("Lesson", "[term_id]")
///             .freezes("Presence", "[lesson_id.term_id]")
///             .requires_none("Lesson", "[status] = 'draft'")
///             .reopen_reason(10),
///     );
/// assert!(term.check().is_ok());
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct PeriodClosing {
    /// The entity of the versions, which [`PeriodClosing::versions_entity`] makes.
    pub versions: String,
    /// The entities whose records belong to the period, each with the path of references that
    /// leads from one of its records to the period.
    pub freezes: Vec<(String, String)>,
    /// What must not exist to approve: records of a frozen entity of the period that meet this.
    pub requires_none: Vec<(String, String)>,
    /// How many characters a reason to reopen needs at least.
    pub reopen_reason: usize,
}

impl PeriodClosing {
    /// Versions kept in an entity of this name.
    pub fn new(versions: impl Into<String>) -> Self {
        Self { versions: versions.into(), freezes: Vec::new(), requires_none: Vec::new(), reopen_reason: 1 }
    }

    /// The records of `entity` belong to the period: `path` is the reference that leads to it, as
    /// `[term_id]`, or the references through other records, as `[lesson_id.term_id]`.
    pub fn freezes(mut self, entity: impl Into<String>, path: impl Into<String>) -> Self {
        self.freezes.push((entity.into(), path.into()));
        self
    }

    /// The period is approved only when no record of `entity`, which it freezes, meets this.
    pub fn requires_none(mut self, entity: impl Into<String>, expression: impl Into<String>) -> Self {
        self.requires_none.push((entity.into(), expression.into()));
        self
    }

    /// A reason to reopen needs at least this many characters.
    pub fn reopen_reason(mut self, at_least: usize) -> Self {
        self.reopen_reason = at_least.max(1);
        self
    }

    /// The entity that keeps the versions of `period`. Its key is the period's key and the
    /// version, as `7/2`, so two versions with the same number cannot be kept.
    ///
    /// Add it with the other entities: a memory database adds it alone, a SQL one takes it with
    /// `managed` like any other.
    pub fn versions_entity(&self, period: &EntityInfo) -> EntityInfo {
        EntityInfo::new(self.versions.clone())
            .caption(format!("{} approval", period.caption))
            .field(FieldInfo::new("id", FieldKind::Text).key())
            .field(FieldInfo::new("period", FieldKind::Reference(period.name.clone())))
            .field(FieldInfo::new("version", FieldKind::Integer))
            .field(FieldInfo::new("approved_at", FieldKind::DateTime))
            .field(FieldInfo::new("approved_by", FieldKind::Text))
            .field(FieldInfo::new("digest", FieldKind::Text))
            .field(FieldInfo::new("document", FieldKind::Text))
            .field(FieldInfo::new("status", FieldKind::Text))
            .field(FieldInfo::new("reopen_reason", FieldKind::LongText))
    }

    /// What is wrong with it on its own: an expression that does not read, or a precondition over
    /// an entity it does not freeze.
    pub(crate) fn problem(&self) -> Option<String> {
        if let Some((entity, _)) = self.freezes.iter().find(|(_, path)| fields_of(path).is_empty()) {
            return Some(format!("it freezes {entity} by an empty path"));
        }
        for (entity, expression) in &self.requires_none {
            if !self.freezes.iter().any(|(frozen, _)| frozen == entity) {
                return Some(format!("it requires no {entity}, which it does not freeze"));
            }
            if let Err(e) = Criteria::parse(expression) {
                return Some(format!("what it requires of {entity} does not read: {e}"));
            }
        }
        None
    }
}

/// The fields of a path, as `[lesson_id.term_id]` gives `lesson_id` and `term_id`.
fn fields_of(path: &str) -> Vec<&str> {
    path.trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split('.')
        .map(str::trim)
        .filter(|f| !f.is_empty())
        .collect()
}

/// The key of a version of a period.
pub fn closing_key(period: &Value, version: u32) -> Option<String> {
    key_text(period).map(|period| format!("{period}/{version}"))
}

/// Whether periods fit across the entities: what they freeze is there, and each path leads, by
/// references, to the period.
pub fn check_closing_model(entities: &[EntityInfo]) -> Result<(), ModelError> {
    for period in entities {
        let Some(closing) = &period.period_closing else { continue };
        let wrong = |what: String| Err(ModelError(format!("{}: {what}", period.name)));
        for (name, path) in &closing.freezes {
            let Some(mut at) = entities.iter().find(|entity| entity.name == *name) else {
                return wrong(format!("it freezes {name}, which is not an entity"));
            };
            let fields = fields_of(path);
            for (step, field) in fields.iter().enumerate() {
                let Some(FieldKind::Reference(next)) = at.field_named(field).map(|f| &f.kind) else {
                    return wrong(format!("{path} of {name}: {field} is not a reference of {}", at.name));
                };
                if step + 1 == fields.len() {
                    if *next != period.name {
                        return wrong(format!("{path} of {name} leads to {next}, not to {}", period.name));
                    }
                } else if let Some(found) = entities.iter().find(|entity| entity.name == *next) {
                    at = found;
                } else {
                    return wrong(format!("{path} of {name} goes through {next}, which is not an entity"));
                }
            }
        }
    }
    Ok(())
}

/// The versions entities the model asks for and does not have yet, to add to it.
pub fn missing_closing_entities(entities: &[EntityInfo]) -> Vec<EntityInfo> {
    entities
        .iter()
        .filter_map(|period| {
            let closing = period.period_closing.as_ref()?;
            let known = entities.iter().any(|entity| entity.name == closing.versions);
            (!known).then(|| closing.versions_entity(period))
        })
        .collect()
}

fn refused(entity: &EntityInfo, why: impl fmt::Display) -> SpaceError {
    SpaceError::Refused(format!("{}: {why}", entity.caption))
}

fn entity_of<'a>(entities: &'a [EntityInfo], name: &str) -> Result<&'a EntityInfo, SpaceError> {
    entities.iter().find(|entity| entity.name == name).ok_or_else(|| SpaceError::UnknownEntity(name.into()))
}

fn key_value(entity: &EntityInfo, object: &Object) -> Value {
    entity.key_field().map_or(Value::Null, |field| object.get(&field.name))
}

/// A record as it will be once these changes are kept: what waits, or else what is stored.
fn record_after(
    entity: &EntityInfo,
    key: &Value,
    changes: &[Change],
    stored: &mut StoredLookup<'_>,
) -> Result<Option<Object>, SpaceError> {
    let text = key_text(key);
    for change in changes.iter().rev() {
        match change {
            Change::New(object) | Change::Updated(object)
                if object.entity() == entity.name && key_text(&key_value(entity, object)) == text =>
            {
                return Ok(Some(object.clone()));
            }
            Change::Deleted { entity: name, key: gone } if *name == entity.name && key_text(gone) == text => {
                return Ok(None);
            }
            _ => {}
        }
    }
    stored(entity, key)
}

/// The key of the period a record belongs to, by the path, with what waits.
fn period_of(
    entities: &[EntityInfo],
    entity: &EntityInfo,
    object: &Object,
    path: &str,
    changes: &[Change],
    stored: &mut StoredLookup<'_>,
) -> Result<Value, SpaceError> {
    let fields = fields_of(path);
    let mut value = object.get(fields[0]);
    let mut at = entity;
    for (step, field) in fields.iter().enumerate().skip(1) {
        let Some(FieldKind::Reference(next)) = at.field_named(fields[step - 1]).map(|f| &f.kind) else {
            return Ok(Value::Null);
        };
        at = entity_of(entities, next)?;
        let Some(record) = record_after(at, &value, changes, stored)? else { return Ok(Value::Null) };
        value = record.get(field);
    }
    Ok(value)
}

/// How many versions storage has of a period, and the last of them as it will be after the changes.
fn last_version(
    versions: &EntityInfo,
    period: &Value,
    changes: &[Change],
    stored: &mut StoredLookup<'_>,
) -> Result<(u32, Option<Object>), SpaceError> {
    let mut count = 0;
    while let Some(key) = closing_key(period, count + 1) {
        if stored(versions, &Value::Text(key))?.is_none() {
            break;
        }
        count += 1;
    }
    let last = match closing_key(period, count) {
        Some(key) if count > 0 => record_after(versions, &Value::Text(key), changes, stored)?,
        _ => None,
    };
    Ok((count, last))
}

/// Whether a period will be closed once these changes are kept: its last version is current, or a
/// new current one waits.
fn closed_after(
    versions: &EntityInfo,
    period: &Value,
    changes: &[Change],
    stored: &mut StoredLookup<'_>,
) -> Result<bool, SpaceError> {
    let (count, last) = last_version(versions, period, changes, stored)?;
    if last.is_some_and(|last| last.text("status") == CLOSING_CURRENT) {
        return Ok(true);
    }
    let next = closing_key(period, count + 1).map(Value::Text);
    Ok(match next {
        Some(next) => record_after(versions, &next, changes, stored)?
            .is_some_and(|waiting| waiting.text("status") == CLOSING_CURRENT),
        None => false,
    })
}

fn is_digest(text: &str) -> bool {
    text.len() == 64 && text.chars().all(|c| c.is_ascii_hexdigit())
}

/// Whether these changes keep to the closed periods. Whoever keeps records calls this with
/// everything about to be committed and a way to read what storage has now, before writing, as
/// every space here does.
///
/// A record of a period whose last version is current is not added, changed or deleted. A new
/// version comes next in sequence, current, with its hash and document, while the period has none
/// current. A version kept changes only from current to reopened, with the reason, and is never
/// deleted.
pub fn check_closing(
    entities: &[EntityInfo],
    changes: &[Change],
    stored: &mut StoredLookup<'_>,
) -> Result<(), SpaceError> {
    let periods: Vec<(&EntityInfo, &PeriodClosing)> = entities
        .iter()
        .filter_map(|period| period.period_closing.as_ref().map(|closing| (period, closing)))
        .collect();
    if periods.is_empty() {
        return Ok(());
    }
    let mut approving: BTreeSet<(String, String)> = BTreeSet::new();
    for change in changes {
        let name = match change {
            Change::New(object) | Change::Updated(object) => object.entity(),
            Change::Deleted { entity, .. } => entity.as_str(),
        };
        let entity = entity_of(entities, name)?;
        for (period, closing) in &periods {
            let versions = entity_of(entities, &closing.versions)?;
            if closing.versions == entity.name {
                check_version(period, closing, versions, change, changes, stored, &mut approving)?;
            }
            for (_, path) in closing.freezes.iter().filter(|(frozen, _)| *frozen == entity.name) {
                let mut touched = Vec::new();
                match change {
                    Change::New(object) => {
                        touched.push(period_of(entities, entity, object, path, changes, stored)?);
                    }
                    Change::Updated(object) => {
                        touched.push(period_of(entities, entity, object, path, changes, stored)?);
                        if let Some(kept) = stored(entity, &key_value(entity, object))? {
                            touched.push(period_of(entities, entity, &kept, path, &[], stored)?);
                        }
                    }
                    Change::Deleted { key, .. } => {
                        if let Some(kept) = stored(entity, key)? {
                            touched.push(period_of(entities, entity, &kept, path, &[], stored)?);
                        }
                    }
                }
                for key in touched {
                    let Some(text) = key_text(&key) else { continue };
                    if closed_after(versions, &key, changes, stored)? {
                        return Err(refused(
                            period,
                            format!("{text} is approved: reopen it before changing {}", entity.caption),
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

/// The rules of one change to a version.
fn check_version(
    period: &EntityInfo,
    closing: &PeriodClosing,
    versions: &EntityInfo,
    change: &Change,
    changes: &[Change],
    stored: &mut StoredLookup<'_>,
    approving: &mut BTreeSet<(String, String)>,
) -> Result<(), SpaceError> {
    match change {
        Change::Deleted { .. } => Err(refused(versions, "an approval that is kept is not deleted")),
        Change::New(object) => {
            let key = object.get("period");
            let Some(text) = key_text(&key) else {
                return Err(refused(versions, format!("it says no {}", period.caption)));
            };
            if !approving.insert((period.name.clone(), text.clone())) {
                return Err(refused(period, format!("{text} is approved once at a time")));
            }
            if record_after(period, &key, changes, stored)?.is_none() {
                return Err(refused(versions, format!("there is no {} {text}", period.caption)));
            }
            let (count, _) = last_version(versions, &key, &[], stored)?;
            let next = count + 1;
            let own_key = key_text(&key_value(versions, object));
            if object.number("version") != Some(f64::from(next)) || own_key != closing_key(&key, next) {
                return Err(refused(versions, format!("the next version of {text} is {next}")));
            }
            if object.text("status") != CLOSING_CURRENT {
                return Err(refused(versions, "a new approval is current"));
            }
            if !is_digest(&object.text("digest")) || object.text("document").is_empty() {
                return Err(refused(versions, "an approval keeps its document and its hash"));
            }
            let still_current = match closing_key(&key, count) {
                Some(last_key) if count > 0 => {
                    record_after(versions, &Value::Text(last_key), changes, stored)?
                        .is_some_and(|after| after.text("status") == CLOSING_CURRENT)
                }
                _ => false,
            };
            if still_current {
                return Err(refused(period, format!("{text} is approved already: reopen it first")));
            }
            Ok(())
        }
        Change::Updated(object) => {
            let Some(kept) = stored(versions, &key_value(versions, object))? else { return Ok(()) };
            let other_changed = versions
                .fields
                .iter()
                .filter(|field| field.name != "status" && field.name != "reopen_reason")
                .any(|field| kept.get(&field.name) != object.get(&field.name));
            let was = kept.text("status");
            let is = object.text("status");
            let reason_changed = kept.get("reopen_reason") != object.get("reopen_reason");
            if !other_changed && was == is && !reason_changed {
                return Ok(());
            }
            if other_changed || was != CLOSING_CURRENT || is != CLOSING_REOPENED {
                return Err(refused(versions, "an approval that is kept changes only to reopened"));
            }
            if object.text("reopen_reason").trim().chars().count() < closing.reopen_reason {
                return Err(refused(
                    versions,
                    format!("the reason to reopen needs at least {} characters", closing.reopen_reason),
                ));
            }
            Ok(())
        }
    }
}

/// What went wrong approving or reading an approved document.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ClosingError {
    /// The space refused or failed.
    Space(SpaceError),
    /// Where files are kept failed.
    Files(String),
    /// The entity has no period closing, or there is no such version or document.
    Missing(String),
    /// The document kept does not match its hash: it is not given.
    Tampered,
}

impl fmt::Display for ClosingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Space(error) => error.fmt(f),
            Self::Files(why) => write!(f, "the files: {why}"),
            Self::Missing(what) => write!(f, "{what} is missing"),
            Self::Tampered => f.write_str("the document kept does not match its hash"),
        }
    }
}

impl std::error::Error for ClosingError {}

impl From<SpaceError> for ClosingError {
    fn from(error: SpaceError) -> Self {
        Self::Space(error)
    }
}

/// The sha256 of these bytes, in hexadecimal.
pub fn closing_digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

fn closing_of(space: &dyn ObjectSpace, period: &str) -> Result<(EntityInfo, PeriodClosing), ClosingError> {
    let entities = space.entities();
    let info = entities
        .iter()
        .find(|entity| entity.name == period)
        .ok_or_else(|| ClosingError::Missing(format!("the entity {period}")))?;
    let closing = info
        .period_closing
        .clone()
        .ok_or_else(|| ClosingError::Missing(format!("the closing of {period}")))?;
    Ok((info.clone(), closing))
}

/// The versions of a period, first to last, as the space reads them.
pub fn closing_versions(
    space: &dyn ObjectSpace,
    period: &str,
    key: &Value,
) -> Result<Vec<Object>, ClosingError> {
    let (_, closing) = closing_of(space, period)?;
    let mut found = Vec::new();
    while let Some(version_key) = closing_key(key, u32::try_from(found.len() + 1).unwrap_or(u32::MAX)) {
        match space.find(&closing.versions, &Value::Text(version_key))? {
            Some(version) => found.push(version),
            None => break,
        }
    }
    Ok(found)
}

/// Approves a period: checks what it requires, keeps the document and writes the next version,
/// current, with its hash, in one commit. Gives the version kept. Whatever is waiting in the space
/// is committed with it.
pub fn approve(
    space: &mut dyn ObjectSpace,
    files: &dyn AttachmentStore,
    period: &str,
    key: &Value,
    document: &[u8],
    approver: &str,
) -> Result<Object, ClosingError> {
    let (info, closing) = closing_of(space, period)?;
    let text = key_text(key).ok_or_else(|| ClosingError::Missing(format!("the key of {}", info.caption)))?;
    if space.find(period, key)?.is_none() {
        return Err(ClosingError::Missing(format!("{} {text}", info.caption)));
    }
    let entities = space.entities();
    for (name, expression) in &closing.requires_none {
        let entity = entity_of(&entities, name)?;
        let filter = Criteria::parse(expression).map_err(|e| SpaceError::Refused(e.to_string()))?;
        let open = space.query(name, &Query { filter: Some(filter), ..Query::default() })?;
        let path = closing.freezes.iter().find(|(frozen, _)| frozen == name).map(|(_, path)| path.clone());
        let Some(path) = path else { continue };
        for record in open {
            let mut read = |entity: &EntityInfo, key: &Value| space.find(&entity.name, key);
            let belongs = period_of(&entities, entity, &record, &path, &[], &mut read)?;
            if key_text(&belongs).as_deref() == Some(text.as_str()) {
                return Err(ClosingError::Space(refused(
                    &info,
                    format!("{text} still has {} that keep it open", entity.caption),
                )));
            }
        }
    }
    let next = u32::try_from(closing_versions(space, period, key)?.len() + 1).unwrap_or(u32::MAX);
    let digest = closing_digest(document);
    let name = format!("{}-{text}-v{next}", info.name);
    let stored = files.put(&name, document).map_err(ClosingError::Files)?;
    let mut version = space.new_object(&closing.versions)?;
    version.set("id", Value::Text(closing_key(key, next).unwrap_or_default()));
    version.set("period", key.clone());
    version.set("version", Value::Number(f64::from(next)));
    version.set("approved_at", Value::Date(crate::local_now()));
    version.set("approved_by", Value::Text(approver.to_owned()));
    version.set("digest", Value::Text(digest));
    version.set("document", Value::Text(stored.clone()));
    version.set("status", Value::Text(CLOSING_CURRENT.to_owned()));
    let kept = space.insert(version.clone()).and_then(|()| space.commit());
    if let Err(error) = kept {
        space.rollback();
        let _ = files.remove(&stored);
        return Err(error.into());
    }
    Ok(version)
}

/// Reopens a period: its current version becomes reopened with the reason, in one commit, and its
/// records can change again. Gives the version as kept.
pub fn reopen(
    space: &mut dyn ObjectSpace,
    period: &str,
    key: &Value,
    reason: &str,
) -> Result<Object, ClosingError> {
    let (info, _) = closing_of(space, period)?;
    let mut last = closing_versions(space, period, key)?
        .pop()
        .filter(|version| version.text("status") == CLOSING_CURRENT)
        .ok_or_else(|| ClosingError::Missing(format!("a current approval of {}", info.caption)))?;
    last.set("status", Value::Text(CLOSING_REOPENED.to_owned()));
    last.set("reopen_reason", Value::Text(reason.trim().to_owned()));
    let kept = space.save(last.clone()).and_then(|()| space.commit());
    if let Err(error) = kept {
        space.rollback();
        return Err(error.into());
    }
    Ok(last)
}

/// The document of a version, read through the space, so its permissions hold, and checked
/// against the hash kept with it. One that does not match is not given.
pub fn archived_document(
    space: &dyn ObjectSpace,
    files: &dyn AttachmentStore,
    versions: &str,
    key: &Value,
) -> Result<Vec<u8>, ClosingError> {
    let version =
        space.find(versions, key)?.ok_or_else(|| ClosingError::Missing(format!("the approval {key:?}")))?;
    let bytes = files
        .get(&version.text("document"))
        .map_err(ClosingError::Files)?
        .ok_or_else(|| ClosingError::Missing("the approved document".to_owned()))?;
    if closing_digest(&bytes) != version.text("digest") {
        return Err(ClosingError::Tampered);
    }
    Ok(bytes)
}
