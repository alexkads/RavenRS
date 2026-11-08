//! Actions declared as data: on the records selected, they set fields to expressions, check the
//! rules and keep what passes. No code runs but the expressions.

use ravenrs_criteria::{Criteria, Scope};

use crate::{
    Action, ActionCategory, CONTEXT_SAVE, CriteriaMode, EntityInfo, ModelError, Module, Operation, RuleWords,
    Selection, SpaceError, ViewController, Violation,
};

/// An action of an entity that sets fields of the records selected.
///
/// ```
/// use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, RecordAction};
///
/// let task = EntityInfo::new("Task")
///     .field(FieldInfo::new("id", FieldKind::Integer).key())
///     .field(FieldInfo::new("state", FieldKind::Text))
///     .action(
///         RecordAction::new("Finish")
///             .enabled("[state] <> 'done'")
///             .set("state = 'done'")
///             .confirm("Finish the tasks?"),
///     );
/// assert!(task.check().is_ok());
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct RecordAction {
    /// What its button says.
    pub caption: String,
    /// Its id; `Entity.Caption` unless given.
    pub id: Option<String>,
    /// How many records it takes: one or more unless given.
    pub selection: Selection,
    /// An expression every record selected has to meet for it to be on.
    pub enabled: Option<String>,
    /// What it sets: `field = expression; ...`, each read from the record as it was.
    pub set: String,
    /// The question asked before it runs.
    pub confirm: Option<String>,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl RecordAction {
    /// An action with this caption, on one record or more, that sets nothing yet.
    pub fn new(caption: impl Into<String>) -> Self {
        Self {
            caption: caption.into(),
            id: None,
            selection: Selection::RequireMultiple,
            enabled: None,
            set: String::new(),
            confirm: None,
        }
    }

    /// Gives it an id of its own.
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// How many records it takes.
    pub fn selection(mut self, selection: Selection) -> Self {
        self.selection = selection;
        self
    }

    /// On only while every record selected meets this expression.
    pub fn enabled(mut self, expression: impl Into<String>) -> Self {
        self.enabled = Some(expression.into());
        self
    }

    /// What it sets, as `field = expression; ...`.
    pub fn set(mut self, assignments: impl Into<String>) -> Self {
        self.set = assignments.into();
        self
    }

    /// Asks this before it runs.
    pub fn confirm(mut self, question: impl Into<String>) -> Self {
        self.confirm = Some(question.into());
        self
    }

    /// Its id on this entity.
    pub fn id_on(&self, entity: &EntityInfo) -> String {
        self.id.clone().unwrap_or_else(|| format!("{}.{}", entity.name, self.caption))
    }

    /// The action that does it on this entity.
    pub fn to_action(&self, entity: &EntityInfo) -> Action {
        let mut action = Action::simple(self.id_on(entity), self.caption.clone())
            .category(ActionCategory::Edit)
            .target_entity(entity.name.clone())
            .selection(self.selection)
            .requires(Operation::Write);
        if let Some(enabled) = &self.enabled {
            action = action.target_criteria(enabled.clone(), CriteriaMode::TrueForAll);
        }
        if let Some(question) = &self.confirm {
            action = action.confirmation(question.clone());
        }
        action.assign(entity, &self.set)
    }

    /// Says what is wrong with it on this entity: an expression that does not parse, or a field
    /// it has not.
    pub(crate) fn check(&self, entity: &EntityInfo) -> Result<(), ModelError> {
        let id = self.id_on(entity);
        let wrong = |what: String| ModelError(format!("the action {id}: {what}"));
        if let Some(enabled) = &self.enabled {
            Criteria::parse(enabled).map_err(|e| wrong(e.to_string()))?;
        }
        assignments(entity, &self.set).map(drop).map_err(wrong)
    }
}

/// The assignments of an action, with the fields checked against the entity.
pub(crate) fn assignments(entity: &EntityInfo, text: &str) -> Result<Vec<(String, Criteria)>, String> {
    let found = Criteria::assignments(text).map_err(|e| e.to_string())?;
    if found.is_empty() {
        return Err("it sets nothing".to_owned());
    }
    for (field, _) in &found {
        if !entity.fields.iter().any(|known| &known.name == field) {
            return Err(format!("{} has no field {field}", entity.name));
        }
    }
    Ok(found)
}

/// Sets the fields of every record selected, checks each against its rules and keeps them all,
/// or none.
pub(crate) fn assign(
    entity: &EntityInfo,
    set: &[(String, Criteria)],
    cx: &crate::ActionContext,
) -> Result<(), String> {
    let key = entity.key_field().map(|key| key.name.clone()).unwrap_or_default();
    let words = cx.service::<RuleWords>().map(|words| (*words).clone()).unwrap_or_default();
    let environment = crate::environment().at(crate::local_now());
    let mut space = cx.space.borrow_mut();
    let result = (|| {
        for record in &cx.selected {
            let mut stored =
                space.find(&entity.name, &record.get(&key)).map_err(|e| e.to_string())?.ok_or_else(|| {
                    SpaceError::Conflict(format!("a record of {} is gone", entity.name)).to_string()
                })?;
            let values = {
                let scope = Scope::new(&environment).row(&stored);
                set.iter()
                    .map(|(field, expression)| {
                        scope
                            .evaluate(expression)
                            .map(|value| (field, value))
                            .map_err(|e| format!("{field}: {e}"))
                    })
                    .collect::<Result<Vec<_>, String>>()?
            };
            for (field, value) in values {
                stored.set(field, value);
            }
            let wrong: Vec<String> = crate::validate(entity, &stored, CONTEXT_SAVE, &*space, &words)
                .into_iter()
                .filter(Violation::is_error)
                .map(|violation| violation.message)
                .collect();
            if !wrong.is_empty() {
                return Err(wrong.join("; "));
            }
            space.save(stored).map_err(|e| e.to_string())?;
        }
        space.commit().map_err(|e| e.to_string())
    })();
    if result.is_err() {
        space.rollback();
    }
    drop(space);
    if result.is_ok() {
        cx.records_changed();
    }
    result
}

struct Declared {
    actions: Vec<Action>,
}

impl ViewController for Declared {
    fn id(&self) -> &str {
        "RecordActions"
    }

    fn actions(&self) -> Vec<Action> {
        self.actions.clone()
    }
}

/// What gives the entities their declared actions. It comes by itself.
#[derive(Debug)]
pub(crate) struct RecordActionModule(pub(crate) Vec<EntityInfo>);

impl Module for RecordActionModule {
    fn name(&self) -> &str {
        "RecordActions"
    }

    fn view_controllers(&self) -> Vec<Box<dyn ViewController>> {
        let actions = self
            .0
            .iter()
            .flat_map(|entity| entity.actions.iter().map(move |declared| declared.to_action(entity)))
            .collect();
        vec![Box::new(Declared { actions })]
    }
}
