//! Requests decided step by step, as an optional `ravenrs-app` package.
//!
//! The application says which steps a request goes through and who may decide each one; the
//! package keeps the request, every decision and an event for each decision. A decision is
//! recorded once however many times it is sent, one taken on an old version of the request is
//! refused, and its event is written **in the same commit**: when the decision is refused, no event
//! exists to be sent.
//!
//! [`Dispatcher`] hands the events to an [`EventSink`] and never sends blindly again: an event sent
//! with no answer is checked with [`EventSink::status`] first, and stays to be reconciled when the
//! sink cannot tell.
//!
//! Nothing here belongs to a sector. Steps, roles and texts are the application's, and two
//! applications can install the same package with different ones.

use std::fmt;
use std::rc::Rc;

use ravenrs_app::{
    Criteria, EntityInfo, EnumOption, FieldInfo, FieldKind, Identity, Module, Object, ObjectSpace, Query,
    SpaceError, Value,
};

pub use ravenrs_app::{CAN_TELL_STATUS, EventSink, Provider};

/// The entity of requests.
pub const REQUEST: &str = "Request";
/// The entity of decisions.
pub const DECISION: &str = "Decision";
/// The entity of events waiting to be sent, written with the change that raised them.
pub const OUTBOX: &str = "OutboxEvent";

/// The state of a request no step is deciding anymore because it was approved.
pub const APPROVED: &str = "approved";
/// The state of a request one step rejected.
pub const REJECTED: &str = "rejected";

/// A step a request goes through, and the role that decides it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    /// What the state of a request on this step is called.
    pub code: String,
    /// What people read.
    pub caption: String,
    /// The role whose members decide it. Nobody else does, whatever other role they have.
    pub decided_by: String,
}

impl Step {
    /// A step with this code and caption, decided by members of this role.
    pub fn new(code: impl Into<String>, caption: impl Into<String>, decided_by: impl Into<String>) -> Self {
        Self { code: code.into(), caption: caption.into(), decided_by: decided_by.into() }
    }
}

/// A rule of the application about one decision, beyond the role of the step: an error says why it
/// is refused.
pub type Policy = Rc<dyn Fn(&Identity, &Object, &Step) -> Result<(), String>>;

/// A policy that refuses decisions taken by whoever made the request.
pub fn forbid_self_decision() -> Policy {
    Rc::new(|identity, request, _| {
        if request.get("requester") == Value::Text(identity.name.clone()) {
            Err("Whoever made a request does not decide it.".to_owned())
        } else {
            Ok(())
        }
    })
}

/// The package, with the steps and the rules of one application.
#[derive(Clone)]
pub struct RequestsModule {
    steps: Vec<Step>,
    policies: Vec<Policy>,
}

impl fmt::Debug for RequestsModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RequestsModule")
            .field("steps", &self.steps)
            .field("policies", &self.policies.len())
            .finish()
    }
}

impl RequestsModule {
    /// The package with these steps, in order. There has to be at least one.
    pub fn new(steps: Vec<Step>) -> Result<Self, String> {
        if steps.is_empty() {
            return Err("A request needs at least one step.".to_owned());
        }
        Ok(Self { steps, policies: Vec::new() })
    }

    /// Adds a rule every decision has to pass, besides the role of its step.
    pub fn policy(mut self, policy: Policy) -> Self {
        self.policies.push(policy);
        self
    }

    /// The steps, in order.
    pub fn steps(&self) -> &[Step] {
        &self.steps
    }

    /// The entities it declares: requests, decisions and events.
    pub fn declared_entities(&self) -> Vec<EntityInfo> {
        let mut states: Vec<EnumOption> =
            self.steps.iter().map(|step| EnumOption::new(step.code.clone(), step.caption.clone())).collect();
        states.push(EnumOption::new(APPROVED, "Approved"));
        states.push(EnumOption::new(REJECTED, "Rejected"));
        let request = EntityInfo::new(REQUEST)
            .field(FieldInfo::new("id", FieldKind::Integer).key())
            .field(FieldInfo::new("title", FieldKind::Text).default_property())
            .field(FieldInfo::new("requester", FieldKind::Text))
            .field(FieldInfo::new("unit", FieldKind::Text))
            .field(FieldInfo::new("state", FieldKind::Enum(states)))
            .field(FieldInfo::new("version", FieldKind::Integer))
            .field(FieldInfo::new("document", FieldKind::Text));
        let outcomes = vec![EnumOption::new("approve", "Approve"), EnumOption::new("reject", "Reject")];
        let decision = EntityInfo::new(DECISION)
            .field(FieldInfo::new("id", FieldKind::Text).key())
            .field(FieldInfo::new("request", FieldKind::Integer))
            .field(FieldInfo::new("step", FieldKind::Text))
            .field(FieldInfo::new("decider", FieldKind::Text))
            .field(FieldInfo::new("outcome", FieldKind::Enum(outcomes)))
            .field(FieldInfo::new("reason", FieldKind::LongText))
            .field(FieldInfo::new("at", FieldKind::Integer));
        let event_states = vec![
            EnumOption::new("pending", "Pending"),
            EnumOption::new("unknown", "To reconcile"),
            EnumOption::new("confirmed", "Confirmed"),
            EnumOption::new("failed", "Failed"),
        ];
        let outbox = EntityInfo::new(OUTBOX)
            .field(FieldInfo::new("key", FieldKind::Text).key())
            .field(FieldInfo::new("kind", FieldKind::Text))
            .field(FieldInfo::new("payload", FieldKind::LongText))
            .field(FieldInfo::new("state", FieldKind::Enum(event_states)))
            .field(FieldInfo::new("attempts", FieldKind::Integer));
        vec![request, decision, outbox]
    }

    fn step(&self, code: &str) -> Option<(usize, &Step)> {
        self.steps.iter().enumerate().find(|(_, step)| step.code == code)
    }

    /// Makes a request on its first step, as this person.
    pub fn submit(
        &self,
        space: &mut dyn ObjectSpace,
        identity: &Identity,
        id: i64,
        title: &str,
        unit: &str,
    ) -> Result<Object, SpaceError> {
        let mut request = space.new_object(REQUEST)?;
        request.set("id", id as f64);
        request.set("title", title);
        request.set("requester", identity.name.clone());
        request.set("unit", unit);
        request.set("state", self.steps[0].code.clone());
        request.set("version", 1.0);
        space.save(request.clone())?;
        commit_or_rollback(space)?;
        Ok(request)
    }

    /// Decides the step a request is on, as this person.
    ///
    /// `decision.id` makes it idempotent: sending the same decision again changes nothing and says
    /// so. `decision.version` is the version of the request the person saw; another one means
    /// someone decided in between, and the decision is refused as a conflict. The decision, the
    /// request moving on and its event are one commit.
    pub fn decide(
        &self,
        space: &mut dyn ObjectSpace,
        identity: &Identity,
        decision: &Decision,
        at: i64,
    ) -> Result<Decided, SpaceError> {
        if space.find(DECISION, &Value::Text(decision.id.clone()))?.is_some() {
            return Ok(Decided::AlreadyRecorded);
        }
        let not_there =
            || SpaceError::Denied("This request is not there, or you cannot decide it.".to_owned());
        let mut request =
            space.find(REQUEST, &Value::Number(decision.request as f64))?.ok_or_else(not_there)?;
        let state = text(&request.get("state"));
        let Some((index, step)) = self.step(&state) else {
            return Err(SpaceError::Refused("This request is not waiting for a decision.".to_owned()));
        };
        if request.get("version") != Value::Number(decision.version as f64) {
            return Err(SpaceError::Conflict("Someone decided this request in the meantime.".to_owned()));
        }
        if !identity.roles.iter().any(|role| role.name() == step.decided_by) {
            return Err(SpaceError::Denied(format!("Only {} decides this step.", step.decided_by)));
        }
        for policy in &self.policies {
            policy(identity, &request, step).map_err(SpaceError::Denied)?;
        }
        let next = match decision.outcome {
            Outcome::Reject => REJECTED.to_owned(),
            Outcome::Approve => {
                self.steps.get(index + 1).map_or_else(|| APPROVED.to_owned(), |step| step.code.clone())
            }
        };
        let mut record = space.new_object(DECISION)?;
        record.set("id", decision.id.clone());
        record.set("request", decision.request as f64);
        record.set("step", step.code.clone());
        record.set("decider", identity.name.clone());
        record.set("outcome", decision.outcome.code());
        record.set("reason", decision.reason.clone());
        record.set("at", at as f64);
        request.set("state", next.clone());
        request.set("version", (decision.version + 1) as f64);
        let mut event = space.new_object(OUTBOX)?;
        event.set("key", format!("decision/{}", decision.id));
        event.set("kind", "request.decided");
        event.set(
            "payload",
            format!(
                "request={};step={};outcome={};state={next}",
                decision.request,
                step.code,
                decision.outcome.code()
            ),
        );
        event.set("state", "pending");
        event.set("attempts", 0.0);
        let written =
            space.insert(record).and_then(|()| space.save(request)).and_then(|()| space.insert(event));
        if let Err(error) = written {
            space.rollback();
            return Err(error);
        }
        match commit_or_rollback(space) {
            Ok(()) => Ok(Decided::Recorded { state: next }),
            Err(SpaceError::DuplicateKey { entity, .. }) if entity == DECISION => {
                Ok(Decided::AlreadyRecorded)
            }
            Err(error) => Err(error),
        }
    }
}

impl Module for RequestsModule {
    fn name(&self) -> &str {
        "Requests"
    }

    fn entities(&self) -> Vec<EntityInfo> {
        self.declared_entities()
    }
}

fn commit_or_rollback(space: &mut dyn ObjectSpace) -> Result<(), SpaceError> {
    space.commit().inspect_err(|_| space.rollback())
}

fn text(value: &Value) -> String {
    match value {
        Value::Text(text) => text.clone(),
        _ => String::new(),
    }
}

/// What a decision says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The request moves to the next step, or is approved after the last one.
    Approve,
    /// The request stops, rejected.
    Reject,
}

impl Outcome {
    fn code(self) -> &'static str {
        match self {
            Self::Approve => "approve",
            Self::Reject => "reject",
        }
    }
}

/// A decision someone sends.
#[derive(Clone, Debug)]
pub struct Decision {
    /// What makes it one decision however many times it is sent.
    pub id: String,
    /// The key of the request.
    pub request: i64,
    /// The version of the request the person saw when deciding.
    pub version: i64,
    /// Approve or reject.
    pub outcome: Outcome,
    /// Why, in their words.
    pub reason: String,
}

/// What became of a decision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decided {
    /// It was kept, and the request is now on this state.
    Recorded {
        /// The state the request moved to.
        state: String,
    },
    /// The same decision was already kept; nothing changed.
    AlreadyRecorded,
}

/// Hands the events waiting in one organization to an [`EventSink`].
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
    /// For this organization, giving up on an event after this many failed attempts.
    pub fn new(organization: impl Into<String>, max_attempts: u32) -> Self {
        Self { organization: organization.into(), max_attempts: max_attempts.max(1) }
    }

    /// The key the destination sees: the organization's name and the event's own key, so that two
    /// organizations with the same local keys never share an effect.
    pub fn key_of(&self, event: &str) -> String {
        format!("{}/{event}", self.organization)
    }

    /// Sends what is pending and checks what was sent with no answer.
    pub fn run(&self, space: &mut dyn ObjectSpace, sink: &dyn EventSink) -> Result<Report, SpaceError> {
        let waiting = Criteria::parse("[state] = 'pending' Or [state] = 'unknown'")
            .map_err(|e| SpaceError::BadQuery(e.to_string()))?;
        let events = space.query(OUTBOX, &Query::all().filter(waiting).sort("key", true))?;
        let mut report = Report::default();
        for mut event in events {
            let key = self.key_of(&text(&event.get("key")));
            if text(&event.get("state")) == "unknown" {
                if !sink.can(CAN_TELL_STATUS) {
                    report.unknown += 1;
                    continue;
                }
                match sink.status(&key) {
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
            let state = match sink.send(&key, &text(&event.get("payload"))) {
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
        commit_or_rollback(space)?;
        Ok(report)
    }
}
