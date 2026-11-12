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

use ravenrs_criteria::Scope;

use ravenrs_app::{
    Access, Criteria, EntityInfo, EnumOption, FieldInfo, FieldKind, Identity, Module, ModuleManifest, Object,
    ObjectSpace, ObjectSpaceExt, Operation, Query, Role, SpaceError, Value,
};

pub use ravenrs_app::{
    CAN_TELL_STATUS, Dispatcher, Emitted, Event, EventSink, OUTBOX_EVENT as OUTBOX, OutboxModule, Provider,
    Report,
};

/// The entity of requests.
pub const REQUEST: &str = "Request";
/// The entity of decisions.
pub const DECISION: &str = "Decision";
/// The entity of tasks: one per request and step, open until the step is decided.
pub const TASK: &str = "RequestTask";
/// The entity of delegations: a role lent to someone else until a moment.
pub const DELEGATION: &str = "Delegation";

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
    /// The role whose members decide it. Nobody else does, whatever other role they have, unless
    /// a member lent it with [`RequestsModule::delegate`].
    pub decided_by: String,
    /// An expression of `ravenrs-criteria` over the request that has to hold for a decision, with
    /// `?CurrentUserName` and `?CurrentUserId` as the person deciding.
    pub when: Option<String>,
    /// How long the step has, in seconds from when its task opens.
    pub due_in: Option<i64>,
}

impl Step {
    /// A step with this code and caption, decided by members of this role.
    pub fn new(code: impl Into<String>, caption: impl Into<String>, decided_by: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            caption: caption.into(),
            decided_by: decided_by.into(),
            when: None,
            due_in: None,
        }
    }

    /// Only decided when this expression holds over the request, such as
    /// `[requester] <> ?CurrentUserName`.
    pub fn when(mut self, expression: impl Into<String>) -> Self {
        self.when = Some(expression.into());
        self
    }

    /// Gives the step this many seconds from when its task opens.
    pub fn due_in(mut self, seconds: i64) -> Self {
        self.due_in = Some(seconds);
        self
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

    /// The entities it declares: requests, decisions, tasks and delegations. Events are the
    /// framework's, from [`OutboxModule`], which this package depends on.
    ///
    /// The list of tasks shows the open ones: with [`task_reader`] on each role, it is everyone's
    /// own inbox.
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
        let task_states = vec![EnumOption::new("open", "Open"), EnumOption::new("done", "Done")];
        let task = EntityInfo::new(TASK)
            .field(FieldInfo::new("id", FieldKind::Text).key().in_list(false))
            .field(FieldInfo::new("title", FieldKind::Text).default_property())
            .field(FieldInfo::new("request", FieldKind::Integer))
            .field(FieldInfo::new("step", FieldKind::Text))
            .field(FieldInfo::new("role", FieldKind::Text))
            .field(FieldInfo::new("state", FieldKind::Enum(task_states)))
            .field(FieldInfo::new("due", FieldKind::Integer))
            .field(FieldInfo::new("decided_by", FieldKind::Text))
            .list_criteria("[state] = 'open'");
        let delegation = EntityInfo::new(DELEGATION)
            .field(FieldInfo::new("id", FieldKind::Text).key())
            .field(FieldInfo::new("delegator", FieldKind::Text))
            .field(FieldInfo::new("delegate", FieldKind::Text).default_property())
            .field(FieldInfo::new("role", FieldKind::Text))
            .field(FieldInfo::new("until", FieldKind::Integer));
        vec![request, decision, task, delegation]
    }

    fn step(&self, code: &str) -> Option<(usize, &Step)> {
        self.steps.iter().enumerate().find(|(_, step)| step.code == code)
    }

    /// Makes a request on its first step, as this person, and opens the task of that step.
    pub fn submit(
        &self,
        space: &mut dyn ObjectSpace,
        identity: &Identity,
        id: i64,
        title: &str,
        unit: &str,
        at: i64,
    ) -> Result<Object, SpaceError> {
        let mut request = space.new_object(REQUEST)?;
        request.set("id", id as f64);
        request.set("title", title);
        request.set("requester", identity.name.clone());
        request.set("unit", unit);
        request.set("state", self.steps[0].code.clone());
        request.set("version", 1.0);
        let written =
            space.save(request.clone()).and_then(|()| open_task(space, &request, &self.steps[0], at));
        if let Err(error) = written {
            space.rollback();
            return Err(error);
        }
        commit_or_rollback(space)?;
        Ok(request)
    }

    /// Decides the step a request is on, as this person.
    ///
    /// `decision.id` makes it idempotent: sending the same decision again changes nothing and says
    /// so. `organization` is whose event it is, until the organization comes from the context of the
    /// space (#225). `decision.version` is the version of the request the person saw; another one means
    /// someone decided in between, and the decision is refused as a conflict. The decision, the
    /// request moving on and its event are one commit.
    pub fn decide(
        &self,
        space: &mut dyn ObjectSpace,
        organization: &str,
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
        if !self.roles_of(space, identity, at)?.contains(&step.decided_by) {
            return Err(SpaceError::Denied(format!("Only {} decides this step.", step.decided_by)));
        }
        if let Some(expression) = &step.when {
            let criteria = Criteria::parse(expression).map_err(|e| SpaceError::BadQuery(e.to_string()))?;
            let environment = ravenrs_app::environment()
                .parameter("CurrentUserName", identity.name.clone())
                .parameter("CurrentUserId", identity.key.clone());
            match Scope::new(&environment).row(&request).evaluate(&criteria) {
                Ok(Value::Bool(true)) => {}
                _ => {
                    return Err(SpaceError::Denied(format!(
                        "The rule of {} does not allow this decision.",
                        step.caption
                    )));
                }
            }
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
        let payload = format!(
            "request={};step={};outcome={};state={next}",
            decision.request,
            step.code,
            decision.outcome.code()
        );
        let event = Event::new(organization, "request.decided", format!("decision/{}", decision.id), payload);
        let task_key = Value::Text(format!("{}/{}", decision.request, step.code));
        let mut task = space.find(TASK, &task_key)?.ok_or_else(not_there)?;
        task.set("state", "done");
        task.set("decided_by", identity.name.clone());
        let following = match decision.outcome {
            Outcome::Approve => self.steps.get(index + 1),
            Outcome::Reject => None,
        };
        let written = space
            .insert(record)
            .and_then(|()| space.save(task))
            .and_then(|()| following.map_or(Ok(()), |next| open_task(space, &request, next, at)))
            .and_then(|()| space.save(request))
            .and_then(|()| space.emit(&event).map(drop));
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

impl RequestsModule {
    /// The roles this person decides with at this moment: their own, and those lent to them by a
    /// delegation still running.
    pub fn roles_of(
        &self,
        space: &dyn ObjectSpace,
        identity: &Identity,
        at: i64,
    ) -> Result<Vec<String>, SpaceError> {
        let mut roles: Vec<String> = identity.roles.iter().map(|role| role.name().to_owned()).collect();
        let name = identity.name.replace('\'', "''");
        let mine = Criteria::parse(&format!("[delegate] = '{name}' And [until] > {at}"))
            .map_err(|e| SpaceError::BadQuery(e.to_string()))?;
        for delegation in space.query(DELEGATION, &Query::all().filter(mine))? {
            let role = text(&delegation.get("role"));
            if !roles.contains(&role) {
                roles.push(role);
            }
        }
        Ok(roles)
    }

    /// Lends a role this person has to someone else until a moment, after which the other person
    /// no longer decides with it. `id` makes it idempotent.
    #[allow(clippy::too_many_arguments)]
    pub fn delegate(
        &self,
        space: &mut dyn ObjectSpace,
        identity: &Identity,
        id: &str,
        to: &str,
        role: &str,
        until: i64,
        at: i64,
    ) -> Result<(), SpaceError> {
        if !identity.roles.iter().any(|own| own.name() == role) {
            return Err(SpaceError::Denied(format!("Only a member of {role} lends it.")));
        }
        if until <= at {
            return Err(SpaceError::Refused("A delegation has to end later than now.".to_owned()));
        }
        if space.find(DELEGATION, &Value::Text(id.to_owned()))?.is_some() {
            return Ok(());
        }
        let mut delegation = space.new_object(DELEGATION)?;
        delegation.set("id", id);
        delegation.set("delegator", identity.name.clone());
        delegation.set("delegate", to);
        delegation.set("role", role);
        delegation.set("until", until as f64);
        space.insert(delegation)?;
        commit_or_rollback(space)
    }

    /// Roles that let this person read the tasks of the roles lent to them and still running, to
    /// add to their identity when they sign in. Deciding checks the end of each delegation again,
    /// so a role kept past it reads nothing new and decides nothing.
    pub fn delegated_roles(
        &self,
        space: &dyn ObjectSpace,
        identity: &Identity,
        at: i64,
    ) -> Result<Vec<Role>, SpaceError> {
        let own: Vec<&str> = identity.roles.iter().map(Role::name).collect();
        Ok(self
            .roles_of(space, identity, at)?
            .into_iter()
            .filter(|role| !own.contains(&role.as_str()))
            .map(|role| Role::new(format!("{role} (lent)")).reads_tasks_of(&role))
            .collect())
    }

    /// The open tasks this person decides at this moment, by their roles and the delegations
    /// still running.
    pub fn tasks_for(
        &self,
        space: &dyn ObjectSpace,
        identity: &Identity,
        at: i64,
    ) -> Result<Vec<Object>, SpaceError> {
        let roles = self.roles_of(space, identity, at)?;
        let open = Criteria::parse("[state] = 'open'").map_err(|e| SpaceError::BadQuery(e.to_string()))?;
        let tasks = space.query(TASK, &Query::all().filter(open).sort("id", true))?;
        Ok(tasks.into_iter().filter(|task| roles.contains(&text(&task.get("role")))).collect())
    }
}

/// Lets members of a role read the tasks of the steps that role decides, so the generated list of
/// tasks is their inbox. Give it to each role that decides a step.
pub fn task_reader(role: Role) -> Role {
    let name = role.name().to_owned();
    role.reads_tasks_of(&name)
}

/// Reading the tasks of a role, added to another role.
trait ReadsTasks {
    fn reads_tasks_of(self, role: &str) -> Role;
}

impl ReadsTasks for Role {
    fn reads_tasks_of(self, role: &str) -> Role {
        let name = role.replace('\'', "''");
        self.object_permission(TASK, &format!("[role] = '{name}'"), [Operation::Read], Access::Allow)
    }
}

fn open_task(space: &mut dyn ObjectSpace, request: &Object, step: &Step, at: i64) -> Result<(), SpaceError> {
    let key = match request.get("id") {
        Value::Number(n) => n as i64,
        _ => 0,
    };
    let mut task = space.new_object(TASK)?;
    task.set("id", format!("{key}/{}", step.code));
    task.set("title", request.get("title"));
    task.set("request", key as f64);
    task.set("step", step.code.clone());
    task.set("role", step.decided_by.clone());
    task.set("state", "open");
    if let Some(due_in) = step.due_in {
        task.set("due", (at + due_in) as f64);
    }
    space.insert(task)
}

impl Module for RequestsModule {
    fn name(&self) -> &str {
        "Requests"
    }

    fn dependencies(&self) -> Vec<String> {
        vec![OutboxModule.name().to_owned()]
    }

    fn entities(&self) -> Vec<EntityInfo> {
        self.declared_entities()
    }

    /// Its version is the crate's; it runs anywhere and brings one job, the dispatch of events.
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new().version(env!("CARGO_PKG_VERSION")).unwrap_or_default().job(DISPATCH_JOB)
    }
}

/// The id of the job that hands events to their destination.
pub const DISPATCH_JOB: &str = "requests.dispatch";

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
