//! Events written in the same commit as the change that raised them, and sent after it.
//!
//! [`ObjectSpaceExt::emit`](crate::ObjectSpaceExt::emit) adds an event to the unit of work in
//! progress: the commit keeps the change and the event together, or neither, so a change that is
//! refused announces nothing. An event is one per organization and key, however many times it is
//! emitted. [`Dispatcher`] then hands what is pending to an [`EventSink`], and never sends blindly
//! again an event whose answer was lost.

use ravenrs_criteria::{Criteria, DateTime};

use crate::{
    CAN_TELL_STATUS, EntityInfo, EnumOption, EventSink, FieldInfo, FieldKind, Module, ObjectSpace, Query,
    SpaceError, Value,
};

/// The entity of events waiting to be sent.
pub const OUTBOX_EVENT: &str = "OutboxEvent";

/// The events waiting to be sent, as an entity: one record per organization and key.
pub fn outbox_entity() -> EntityInfo {
    let states = vec![
        EnumOption::new("pending", "Pending"),
        EnumOption::new("unknown", "To reconcile"),
        EnumOption::new("confirmed", "Confirmed"),
        EnumOption::new("failed", "Failed"),
    ];
    EntityInfo::new(OUTBOX_EVENT)
        .field(FieldInfo::new("id", FieldKind::Text).key().read_only())
        .field(FieldInfo::new("organization", FieldKind::Text).read_only())
        .field(FieldInfo::new("kind", FieldKind::Text).default_property().read_only())
        .field(FieldInfo::new("version", FieldKind::Integer).read_only())
        .field(FieldInfo::new("key", FieldKind::Text).read_only())
        .field(FieldInfo::new("payload", FieldKind::LongText).read_only().in_list(false))
        .field(FieldInfo::new("state", FieldKind::Enum(states)).read_only())
        .field(FieldInfo::new("attempts", FieldKind::Integer).read_only())
        .field(FieldInfo::new("created_at", FieldKind::DateTime).read_only())
}

/// Declares the entity of events, for applications whose modules emit them. Modules that emit
/// depend on it by its name, `Outbox`.
#[derive(Clone, Copy, Debug, Default)]
pub struct OutboxModule;

impl Module for OutboxModule {
    fn name(&self) -> &str {
        "Outbox"
    }

    fn entities(&self) -> Vec<EntityInfo> {
        vec![outbox_entity()]
    }
}

/// An event to emit.
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    /// The organization it belongs to; its key is unique only inside it.
    pub organization: String,
    /// What happened, such as `request.decided`.
    pub kind: String,
    /// The version of the shape of `kind`, for whoever reads the payload.
    pub version: u32,
    /// What makes it one event however many times it is emitted.
    pub key: String,
    /// What the destination receives.
    pub payload: String,
    /// When it was raised; now when not given.
    pub at: Option<DateTime>,
}

impl Event {
    /// An event of this kind, in this organization, under this key.
    pub fn new(
        organization: impl Into<String>,
        kind: impl Into<String>,
        key: impl Into<String>,
        payload: impl Into<String>,
    ) -> Self {
        Self {
            organization: organization.into(),
            kind: kind.into(),
            version: 1,
            key: key.into(),
            payload: payload.into(),
            at: None,
        }
    }

    /// The version of the shape of its kind.
    pub fn version(mut self, version: u32) -> Self {
        self.version = version;
        self
    }

    /// When it was raised.
    pub fn at(mut self, at: DateTime) -> Self {
        self.at = Some(at);
        self
    }

    /// The key of its record, and the key the destination sees: the organization and the event's
    /// own key, so that two organizations with the same local keys never share an effect.
    pub fn id(&self) -> String {
        format!("{}/{}", self.organization, self.key)
    }
}

/// What became of an emitted event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Emitted {
    /// It waits for the commit.
    Added,
    /// The organization already has an event under this key, kept or waiting: nothing was added.
    AlreadyThere,
}

pub(crate) fn emit<S: ObjectSpace + ?Sized>(space: &mut S, event: &Event) -> Result<Emitted, SpaceError> {
    let id = Value::Text(event.id());
    let waiting = space.changes().iter().any(|change| match change {
        crate::Change::New(object) | crate::Change::Updated(object) => {
            object.entity() == OUTBOX_EVENT && object.get("id") == id
        }
        _ => false,
    });
    if waiting || space.find(OUTBOX_EVENT, &id)?.is_some() {
        return Ok(Emitted::AlreadyThere);
    }
    let mut record = space.new_object(OUTBOX_EVENT)?;
    record.set("id", id);
    record.set("organization", event.organization.clone());
    record.set("kind", event.kind.clone());
    record.set("version", f64::from(event.version));
    record.set("key", event.key.clone());
    record.set("payload", event.payload.clone());
    record.set("state", "pending");
    record.set("attempts", 0.0);
    record.set("created_at", Value::Date(event.at.unwrap_or_else(crate::workflow::local_now)));
    space.insert(record)?;
    Ok(Emitted::Added)
}

/// Hands the events waiting in one organization to an [`EventSink`], in this process.
#[derive(Clone, Debug)]
pub struct Dispatcher {
    organization: String,
    max_attempts: u32,
}

/// What one run of a [`Dispatcher`] did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// Events confirmed in this run.
    pub confirmed: usize,
    /// Events that will be tried again.
    pub retry: usize,
    /// Events sent with no answer, to be checked before sending again.
    pub unknown: usize,
    /// Events that failed for good.
    pub failed: usize,
}

impl Dispatcher {
    /// For this organization, giving up on an event after this many temporary failures.
    pub fn new(organization: impl Into<String>, max_attempts: u32) -> Self {
        Self { organization: organization.into(), max_attempts: max_attempts.max(1) }
    }

    /// Sends what is pending and checks what was sent with no answer.
    ///
    /// A temporary failure is tried again on a later run, up to the limit; a refusal, a missing
    /// capability or any other failure fails at once. An event whose answer was lost is asked
    /// about with [`EventSink::status`] first, and stays to be reconciled when the sink cannot tell.
    pub fn run(&self, space: &mut dyn ObjectSpace, sink: &dyn EventSink) -> Result<Report, SpaceError> {
        let organization = self.organization.replace('\'', "''");
        let waiting = Criteria::parse(&format!(
            "[organization] = '{organization}' And ([state] = 'pending' Or [state] = 'unknown')"
        ))
        .map_err(|e| SpaceError::BadQuery(e.to_string()))?;
        let events = space.query(OUTBOX_EVENT, &Query::all().filter(waiting).sort("id", true))?;
        let mut report = Report::default();
        for mut event in events {
            let id = text(&event.get("id"));
            if text(&event.get("state")) == "unknown" {
                if !sink.can(CAN_TELL_STATUS) {
                    report.unknown += 1;
                    continue;
                }
                match sink.status(&id) {
                    Ok(true) => {
                        event.set("state", "confirmed");
                        report.confirmed += 1;
                        space.save(event)?;
                        continue;
                    }
                    Ok(false) => {}
                    Err(_) => {
                        report.unknown += 1;
                        continue;
                    }
                }
            }
            let attempts = match event.get("attempts") {
                Value::Number(n) => n as u32,
                _ => 0,
            } + 1;
            event.set("attempts", f64::from(attempts));
            let state = match sink.send(&id, &text(&event.get("payload"))) {
                Ok(()) => {
                    report.confirmed += 1;
                    "confirmed"
                }
                Err(error) if error.may_have_happened() => {
                    report.unknown += 1;
                    "unknown"
                }
                Err(error) if error.is_temporary() && attempts < self.max_attempts => {
                    report.retry += 1;
                    "pending"
                }
                Err(_) => {
                    report.failed += 1;
                    "failed"
                }
            };
            event.set("state", state);
            space.save(event)?;
        }
        space.commit().inspect_err(|_| space.rollback())?;
        Ok(report)
    }
}

fn text(value: &Value) -> String {
    match value {
        Value::Text(text) => text.clone(),
        _ => String::new(),
    }
}
