//! Actions declared as data: on the records selected, they set fields to expressions, check the
//! rules and keep what passes. No code runs but the expressions.

use ravenrs_criteria::Criteria;

use crate::action::assignments;
pub use crate::entity::record_actions::RecordAction;
use crate::{
    Action, ActionCategory, CriteriaMode, EntityInfo, ModelError, Module, Operation, Selection,
    ViewController,
};

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
