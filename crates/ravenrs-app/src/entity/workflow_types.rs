//! What an entity's workflow says: the states of a field and the moves between them, and reminders.
//! The entity keeps them; the code that runs them lives in its own module.

use crate::space::key_text;
use ravenrs_criteria::{Criteria, Scope};

use crate::{EntityInfo, Object, SpaceError};

/// A way from one state to another.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
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
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
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

/// The field of a record that says when to remind people of it, and what to say.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct Reminder {
    /// The date and time it is due; empty means nothing to remind.
    pub field: String,
    /// The field that says what it is about; nothing means what names the record.
    pub message: Option<String>,
    /// An expression for whom it is, where `?CurrentUserId` and `?CurrentUserName` are the person.
    pub criteria: Option<String>,
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

/// Whether a change keeps to the state machines of its entity. Whoever keeps records calls this
/// with what is stored and what is about to be, before writing, as every space here does.
pub fn check_transitions(entity: &EntityInfo, old: Option<&Object>, new: &Object) -> Result<(), SpaceError> {
    entity.state_machines.iter().try_for_each(|machine| machine.check(entity, old, new))
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl StateMachine {
    pub(crate) fn caption_of<'a>(&'a self, code: &'a str) -> &'a str {
        self.states.iter().find(|(known, _)| known == code).map_or(code, |(_, caption)| caption)
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
