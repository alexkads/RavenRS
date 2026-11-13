//! Events written in the same commit as the change that raised them, and sent after it.
//!
//! [`ObjectSpaceExt::emit`](crate::ObjectSpaceExt::emit) adds an event to the unit of work in
//! progress: the commit keeps the change and the event together, or neither, so a change that is
//! refused announces nothing. An event is one per organization and key, however many times it is
//! emitted. [`Dispatcher`] then hands what is pending to an [`EventSink`], and never sends blindly
//! again an event whose answer was lost.
//!
//! Before each send the event is kept as `sending`, with a deadline: that is what holds the record
//! while one run sends it, and what is left when a run stops between the send and the answer. Once
//! the deadline passes, the next run treats it as an answer lost and asks the destination first.

use std::rc::Rc;

use ravenrs_criteria::{Criteria, DateTime};

use crate::{
    CAN_TELL_STATUS, EntityInfo, EnumOption, EventSink, FieldInfo, FieldKind, Module, ObjectSpace, Operation,
    Query, RecordAction, SecuredObjectSpace, SecurityContext, SpaceError, Value,
};

/// The action that sends again an event that failed or waits to be reconciled, under the same key.
/// It takes permission to write events.
pub const OUTBOX_REPROCESS: &str = "OutboxEvent.Reprocess";

/// The entity of events waiting to be sent.
pub const OUTBOX_EVENT: &str = "OutboxEvent";

/// The events waiting to be sent, as an entity: one record per organization and key.
pub fn outbox_entity() -> EntityInfo {
    let states = vec![
        EnumOption::new("pending", "Pending"),
        EnumOption::new("sending", "Sending"),
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
        .field(FieldInfo::new("next_at", FieldKind::DateTime).read_only())
        .field(FieldInfo::new("last_error", FieldKind::Text).read_only().in_list(false))
        .action(
            RecordAction::new("Reprocess")
                .id(OUTBOX_REPROCESS)
                .enabled("[state] = 'failed' Or [state] = 'unknown'")
                .set("state = 'pending'; attempts = 0; next_at = [created_at]; last_error = ''")
                .confirm("Send these events again, under the same keys?"),
        )
}

/// Declares the entity of events, for applications whose modules emit them. Modules that emit
/// depend on it by its name, `Outbox`.
#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
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
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
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

#[cfg_attr(feature = "live", ravenrs_live::expose)]
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
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "variants"))]
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
    let at = event.at.unwrap_or_else(crate::workflow::local_now);
    record.set("created_at", Value::Date(at));
    record.set("next_at", Value::Date(at));
    space.insert(record)?;
    Ok(Emitted::Added)
}

/// Hands the events waiting in one organization to an [`EventSink`], in this process.
///
/// ```
/// use ravenrs_app::Dispatcher;
///
/// // Five tries, waiting 30 s, then 60 s, 120 s… between them; a send may take 2 minutes.
/// let dispatcher = Dispatcher::new("T1", 5).backoff(30).lease(120);
/// # let _ = dispatcher;
/// ```
#[derive(Clone, Debug)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Dispatcher {
    organization: String,
    max_attempts: u32,
    backoff: u32,
    lease: u32,
}

/// What one run of a [`Dispatcher`] did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Report {
    /// Events confirmed in this run.
    pub confirmed: usize,
    /// Events that will be tried again.
    pub retry: usize,
    /// Events sent with no answer, to be checked before sending again.
    pub unknown: usize,
    /// Events that failed for good.
    pub failed: usize,
    /// Events left for later: their next try is not due yet, or another run is sending them.
    pub waiting: usize,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Dispatcher {
    /// For this organization, giving up on an event after this many temporary failures. With no
    /// [`backoff`](Self::backoff), a failed event is tried again on the next run.
    pub fn new(organization: impl Into<String>, max_attempts: u32) -> Self {
        Self { organization: organization.into(), max_attempts: max_attempts.max(1), backoff: 0, lease: 300 }
    }

    /// Waits this many seconds after the first temporary failure, and twice as long after each one
    /// that follows.
    pub fn backoff(mut self, seconds: u32) -> Self {
        self.backoff = seconds;
        self
    }

    /// How long one send may take, in seconds; 300 unless given. A record still `sending` after
    /// that was left by a run that stopped, and is checked with the destination before sending again.
    pub fn lease(mut self, seconds: u32) -> Self {
        self.lease = seconds.max(1);
        self
    }

    /// Sends what is due and checks what was sent with no answer, by the clock of this machine.
    pub fn run(&self, space: &mut dyn ObjectSpace, sink: &dyn EventSink) -> Result<Report, SpaceError> {
        self.run_at(space, sink, crate::workflow::local_now())
    }

    /// As [`run`](Self::run), as someone granted the work: who they are is read again first, so a
    /// worker turned off or no longer granted the events is refused before reading anything.
    pub fn run_as(
        &self,
        context: &Rc<SecurityContext>,
        space: Box<dyn ObjectSpace>,
        sink: &dyn EventSink,
        now: DateTime,
    ) -> Result<Report, SpaceError> {
        context.reload()?;
        let granted = context.with(|permissions| {
            [Operation::Read, Operation::Write].into_iter().all(|op| permissions.can(op, OUTBOX_EVENT, None))
        });
        if !granted {
            return Err(context.denied());
        }
        self.run_at(&mut SecuredObjectSpace::new(space, Rc::clone(context)), sink, now)
    }

    /// Sends what is due at this instant and checks what was sent with no answer.
    ///
    /// Each event is first kept as `sending`, and only then sent, so a run that stops halfway
    /// leaves it to be reconciled, never to be sent blindly. A temporary failure is tried again
    /// after the backoff, up to the limit; a refusal, a missing capability or any other failure
    /// fails at once. An event whose answer was lost is asked about with [`EventSink::status`]
    /// first, and stays to be reconciled when the sink cannot tell.
    pub fn run_at(
        &self,
        space: &mut dyn ObjectSpace,
        sink: &dyn EventSink,
        now: DateTime,
    ) -> Result<Report, SpaceError> {
        let organization = self.organization.replace('\'', "''");
        let waiting = Criteria::parse(&format!(
            "[organization] = '{organization}' And [state] In ('pending', 'sending', 'unknown')"
        ))
        .map_err(|e| SpaceError::BadQuery(e.to_string()))?;
        let events = space.query(OUTBOX_EVENT, &Query::all().filter(waiting).sort("id", true))?;
        let mut report = Report::default();
        for event in events {
            self.one(space, sink, now, event, &mut report).inspect_err(|_| space.rollback())?;
        }
        Ok(report)
    }

    fn one(
        &self,
        space: &mut dyn ObjectSpace,
        sink: &dyn EventSink,
        now: DateTime,
        mut event: crate::Object,
        report: &mut Report,
    ) -> Result<(), SpaceError> {
        let id = text(&event.get("id"));
        let due = match event.get("next_at") {
            Value::Date(at) => at <= now,
            _ => true,
        };
        let state = text(&event.get("state"));
        if !due {
            report.waiting += 1;
            return Ok(());
        }
        if state == "unknown" || state == "sending" {
            if !sink.can(CAN_TELL_STATUS) {
                report.unknown += 1;
                if state == "unknown" {
                    return Ok(());
                }
                return keep(
                    space,
                    event,
                    "unknown",
                    None,
                    Some("the destination cannot tell what it received"),
                );
            }
            match sink.status(&id) {
                Ok(true) => {
                    report.confirmed += 1;
                    return keep(space, event, "confirmed", None, None);
                }
                Ok(false) => {}
                Err(error) => {
                    report.unknown += 1;
                    return keep(space, event, "unknown", None, Some(&error.to_string()));
                }
            }
        }
        let attempts = match event.get("attempts") {
            Value::Number(n) => n as u32,
            _ => 0,
        } + 1;
        // Kept before the send: the record is held, and a stop from here on is reconciled.
        event.set("attempts", f64::from(attempts));
        event.set("state", "sending");
        event.set("next_at", Value::Date(later(now, i64::from(self.lease))));
        space.save(event.clone())?;
        space.commit()?;
        let event = space
            .find(OUTBOX_EVENT, &Value::Text(id.clone()))?
            .ok_or_else(|| SpaceError::Conflict(format!("the event {id} is gone")))?;
        match sink.send(&id, &text(&event.get("payload"))) {
            Ok(()) => {
                report.confirmed += 1;
                keep(space, event, "confirmed", None, None)
            }
            Err(error) if error.may_have_happened() => {
                report.unknown += 1;
                keep(space, event, "unknown", None, Some(&error.to_string()))
            }
            Err(error) if error.is_temporary() && attempts < self.max_attempts => {
                report.retry += 1;
                let wait = i64::from(self.backoff) << (attempts - 1).min(20);
                keep(space, event, "pending", Some(later(now, wait)), Some(&error.to_string()))
            }
            Err(error) => {
                report.failed += 1;
                keep(space, event, "failed", None, Some(&error.to_string()))
            }
        }
    }
}

/// Keeps the event in this state and commits.
fn keep(
    space: &mut dyn ObjectSpace,
    mut event: crate::Object,
    state: &str,
    next_at: Option<DateTime>,
    error: Option<&str>,
) -> Result<(), SpaceError> {
    event.set("state", state);
    event.set("next_at", next_at.map_or(Value::Null, Value::Date));
    event.set("last_error", error.unwrap_or_default());
    space.save(event)?;
    space.commit()
}

fn later(at: DateTime, seconds: i64) -> DateTime {
    let total = i64::from(at.seconds) + seconds;
    DateTime { days: at.days + total.div_euclid(86_400), seconds: total.rem_euclid(86_400) as u32 }
}

fn text(value: &Value) -> String {
    match value {
        Value::Text(text) => text.clone(),
        _ => String::new(),
    }
}
