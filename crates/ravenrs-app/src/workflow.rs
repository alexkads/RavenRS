//! States a record goes through, and only by the ways declared. It is refused where records are
//! kept, so it holds with no screen around it.

use ravenrs_criteria::{Criteria, Scope, Value};

use crate::space::key_text;
use crate::{
    Action, ActionCategory, CriteriaMode, EntityInfo, Module, Object, Operation, Query, Selection,
    SpaceError, ViewController,
};

/// A way from one state to another.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Transition {
    /// The state it leaves.
    pub from: String,
    /// The state it reaches.
    pub to: String,
    /// What its action says.
    pub caption: String,
    /// An expression the record has to meet, as it would be after the change.
    pub when: Option<String>,
}

/// The states of a record, in one of its fields, and the ways between them.
///
/// ```
/// use ravenrs_app::{EntityInfo, EnumOption, FieldInfo, FieldKind, StateMachine};
///
/// let options = vec![EnumOption::new("draft", "Draft"), EnumOption::new("done", "Done")];
/// let entity = EntityInfo::new("Task")
///     .field(FieldInfo::new("id", FieldKind::Integer).key())
///     .field(FieldInfo::new("state", FieldKind::Enum(options)))
///     .state_machine(
///         StateMachine::new("state").state("draft", "Draft").state("done", "Done").start("draft").transition(
///             "draft",
///             "done",
///             "Finish",
///         ),
///     );
/// assert!(entity.check().is_ok());
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct StateMachine {
    /// The field that holds the state: options, text or a reference.
    pub field: String,
    /// The states, as stored and as people see them.
    pub states: Vec<(String, String)>,
    /// Where a new record has to start; nothing means any state.
    pub start: Option<String>,
    /// The ways between states.
    pub transitions: Vec<Transition>,
}

impl StateMachine {
    /// Over this field.
    pub fn new(field: impl Into<String>) -> Self {
        Self { field: field.into(), ..Self::default() }
    }

    /// A state, as stored and as people see it.
    pub fn state(mut self, code: impl Into<String>, caption: impl Into<String>) -> Self {
        self.states.push((code.into(), caption.into()));
        self
    }

    /// Where a new record starts, which also is what it starts with.
    pub fn start(mut self, code: impl Into<String>) -> Self {
        self.start = Some(code.into());
        self
    }

    /// A way between two states, with what its action says.
    pub fn transition(
        mut self,
        from: impl Into<String>,
        to: impl Into<String>,
        caption: impl Into<String>,
    ) -> Self {
        self.transitions.push(Transition {
            from: from.into(),
            to: to.into(),
            caption: caption.into(),
            when: None,
        });
        self
    }

    /// The last way declared only opens for records this expression holds for.
    pub fn when(mut self, expression: impl Into<String>) -> Self {
        if let Some(last) = self.transitions.last_mut() {
            last.when = Some(expression.into());
        }
        self
    }

    fn caption_of<'a>(&'a self, code: &'a str) -> &'a str {
        self.states.iter().find(|(known, _)| known == code).map_or(code, |(_, caption)| caption)
    }

    pub(crate) fn problem(&self) -> Option<String> {
        let known = |code: &str| self.states.iter().any(|(state, _)| state == code);
        if self.states.is_empty() {
            return Some(format!("the states of {} are not declared", self.field));
        }
        if let Some(start) = self.start.as_deref().filter(|start| !known(start)) {
            return Some(format!("{} starts in {start}, which is not one of its states", self.field));
        }
        self.transitions.iter().find_map(|transition| {
            [&transition.from, &transition.to]
                .into_iter()
                .find(|state| !known(state))
                .map(|state| format!("{} goes through {state}, which is not one of its states", self.field))
                .or_else(|| {
                    let expression = transition.when.as_deref()?;
                    Criteria::parse(expression).err().map(|error| format!("{}: {error}", self.field))
                })
        })
    }

    /// Whether the record can make this change: to be called with the record as storage has it, or
    /// nothing if it is new, and as it is about to be.
    fn check(&self, entity: &EntityInfo, old: Option<&Object>, new: &Object) -> Result<(), SpaceError> {
        let text = |object: &Object| key_text(&object.get(&self.field)).unwrap_or_default();
        let to = text(new);
        let refused = |why: String| Err(SpaceError::Refused(format!("{}: {why}", entity.caption)));
        let Some(old) = old else {
            if !self.states.iter().any(|(state, _)| *state == to) {
                return refused(format!("{to} is not a state"));
            }
            return match &self.start {
                Some(start) if *start != to => {
                    refused(format!("a new record starts in {}", self.caption_of(start)))
                }
                _ => Ok(()),
            };
        };
        let from = text(old);
        if from == to {
            return Ok(());
        }
        let environment = crate::environment();
        let mut after = new.clone();
        entity.compute(&mut after, &environment);
        let opens = self.transitions.iter().filter(|way| way.from == from && way.to == to).any(|way| {
            way.when.as_deref().is_none_or(|expression| {
                // One that does not read, or fails on the record, does not open.
                Criteria::parse(expression)
                    .ok()
                    .is_some_and(|criteria| Scope::new(&environment).row(&after).matches(&criteria))
            })
        });
        if opens {
            Ok(())
        } else {
            refused(format!("it cannot go from {} to {}", self.caption_of(&from), self.caption_of(&to)))
        }
    }
}

/// Whether a change keeps to the state machines of its entity. Whoever keeps records calls this
/// with what is stored and what is about to be, before writing, as every space here does.
pub fn check_transitions(entity: &EntityInfo, old: Option<&Object>, new: &Object) -> Result<(), SpaceError> {
    entity.state_machines.iter().try_for_each(|machine| machine.check(entity, old, new))
}

/// The field of a record that says when to remind people of it, and what to say.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Reminder {
    /// The date and time it is due; empty means nothing to remind.
    pub field: String,
    /// The field that says what it is about; nothing means what names the record.
    pub message: Option<String>,
    /// An expression for whom it is, where `?CurrentUserId` and `?CurrentUserName` are the person.
    pub criteria: Option<String>,
}

impl Reminder {
    /// Due at the time in this field.
    pub fn new(field: impl Into<String>) -> Self {
        Self { field: field.into(), message: None, criteria: None }
    }

    /// Says what this field holds.
    pub fn message(mut self, field: impl Into<String>) -> Self {
        self.message = Some(field.into());
        self
    }

    /// Only for records this expression holds for.
    pub fn criteria(mut self, expression: impl Into<String>) -> Self {
        self.criteria = Some(expression.into());
        self
    }
}

/// The date and time on this machine, as people read it on a clock.
pub fn local_now() -> ravenrs_criteria::DateTime {
    #[cfg(target_arch = "wasm32")]
    {
        ravenrs_criteria::DateTime::now()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let utc = ravenrs_criteria::DateTime::now();
        let offset = i64::from(jiff::tz::TimeZone::system().to_offset(jiff::Timestamp::now()).seconds());
        let seconds = i64::from(utc.seconds) + offset;
        ravenrs_criteria::DateTime {
            days: utc.days + seconds.div_euclid(86_400),
            seconds: seconds.rem_euclid(86_400) as u32,
        }
    }
}

struct Transitions {
    actions: Vec<Action>,
}

impl ViewController for Transitions {
    fn id(&self) -> &str {
        "StateTransitions"
    }

    fn actions(&self) -> Vec<Action> {
        self.actions.clone()
    }
}

fn action_of(entity: &EntityInfo, machine: &StateMachine, way: &Transition) -> Action {
    let (field, to) = (machine.field.clone(), way.to.clone());
    let key = entity.key_field().map(|key| key.name.clone()).unwrap_or_default();
    let name = entity.name.clone();
    let id = format!("{}.{}.{}.{}", entity.name, machine.field, way.from, way.to);
    let mut opens = format!("[{field}] = \'{}\'", way.from.replace('\'', "\'\'"));
    if let Some(when) = &way.when {
        opens = format!("({opens}) And ({when})");
    }
    Action::simple(id, way.caption.clone())
        .category(ActionCategory::Edit)
        .target_entity(entity.name.clone())
        .selection(Selection::RequireMultiple)
        .target_criteria(opens, CriteriaMode::TrueForAll)
        .requires(Operation::Write)
        .on_execute(move |cx| {
            let mut space = cx.space.borrow_mut();
            let result = (|| {
                for record in &cx.selected {
                    let mut stored = space
                        .find(&name, &record.get(&key))?
                        .ok_or_else(|| SpaceError::Conflict(format!("a record of {name} is gone")))?;
                    stored.set(&field, Value::Text(to.clone()));
                    space.save(stored)?;
                }
                space.commit()
            })();
            if let Err(error) = result {
                space.rollback();
                return Err(error.to_string());
            }
            drop(space);
            cx.records_changed();
            Ok(())
        })
}

/// What gives the entities with state machines their actions. It comes by itself.
#[derive(Debug)]
pub(crate) struct StateMachineModule(pub(crate) Vec<EntityInfo>);

impl Module for StateMachineModule {
    fn name(&self) -> &str {
        "StateMachines"
    }

    fn view_controllers(&self) -> Vec<Box<dyn ViewController>> {
        let actions = self
            .0
            .iter()
            .flat_map(|entity| {
                entity.state_machines.iter().flat_map(move |machine| {
                    machine.transitions.iter().map(move |way| action_of(entity, machine, way))
                })
            })
            .collect();
        vec![Box::new(Transitions { actions })]
    }
}

/// The records of an entity with a reminder that have a time set, for this person, as this space
/// shows them. What is due by now is up to whoever asks, as they know the time.
pub fn reminders_of(
    space: &dyn crate::ObjectSpace,
    entity: &EntityInfo,
    environment: &ravenrs_criteria::Environment,
) -> Vec<Object> {
    let Some(reminder) = &entity.reminder else { return Vec::new() };
    let found = space.query(&entity.name, &Query::all()).unwrap_or_default();
    let criteria = reminder.criteria.as_deref().and_then(|text| Criteria::parse(text).ok());
    found
        .into_iter()
        .filter(|record| matches!(record.get(&reminder.field), Value::Date(_)))
        .filter(|record| {
            criteria.as_ref().is_none_or(|criteria| Scope::new(environment).row(record).matches(criteria))
        })
        .collect()
}
