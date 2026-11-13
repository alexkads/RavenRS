//! States a record goes through, and only by the ways declared. It is refused where records are
//! kept, so it holds with no screen around it.

use ravenrs_criteria::{Criteria, Scope, Value};

pub use crate::entity::workflow_types::{Reminder, StateMachine, Transition};
use crate::{
    Action, ActionCategory, CriteriaMode, EntityInfo, Module, Object, Operation, Query, Selection,
    SpaceError, ViewController,
};

#[cfg_attr(feature = "live", ravenrs_live::expose(part = "part1"))]
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
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
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
