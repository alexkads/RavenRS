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
            confirm_detail: None,
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

    /// What the question says besides, as "The tasks go to the archive.".
    pub fn confirm_detail(mut self, detail: impl Into<String>) -> Self {
        self.confirm_detail = Some(detail.into());
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
        if let Some(detail) = &self.confirm_detail {
            action = action.confirmation_words(detail.clone(), self.caption.clone());
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

/// The action that amends a record, for each entity that amends another: "Correção de aula" on a
/// closed lesson, "Transferência" on an enrollment still open. It opens a new amendment on its
/// form, already pointing to the record, for the reason to be written; the commit applies it.
/// It is on only while the record can take it, and only for whoever can create the amendment.
fn amend_action(amending: &EntityInfo, amendment: &crate::Amendment, target: Option<&EntityInfo>) -> Action {
    let key_field = target.and_then(EntityInfo::key_field).map(|field| field.name.clone());
    let (entity, reference) = (amending.name.clone(), amendment.reference.clone());
    let mut action = Action::simple(format!("{}.Amend", amending.name), amending.caption.clone())
        .category(ActionCategory::Edit)
        .target_entity(amendment.target.clone())
        .selection(Selection::RequireSingle)
        .icon("pencil")
        .requires_on(amending.name.clone(), Operation::Create)
        .on_execute(move |cx| {
            let record = cx.selected.first().ok_or("this action needs a record")?;
            let field = key_field.as_deref().ok_or("the record has no key")?;
            cx.open_new(entity.clone(), vec![(reference.clone(), record.get(field))]);
            Ok(())
        });
    if let Some(seal) = target.and_then(|target| target.seal.as_ref()) {
        let when = match amendment.kind {
            crate::AmendmentKind::Correction => seal.when.clone(),
            crate::AmendmentKind::Closing => format!("Not ({})", seal.when),
        };
        action = action.target_criteria(when, CriteriaMode::TrueForAll);
    }
    action
}

/// What gives each sealed entity the actions of its amendments. It comes by itself.
#[derive(Debug)]
pub(crate) struct AmendmentActionModule(pub(crate) Vec<EntityInfo>);

impl Module for AmendmentActionModule {
    fn name(&self) -> &str {
        "AmendmentActions"
    }

    fn view_controllers(&self) -> Vec<Box<dyn ViewController>> {
        let actions = self
            .0
            .iter()
            .filter_map(|amending| {
                let amendment = amending.amendment.as_ref()?;
                let target = self.0.iter().find(|entity| entity.name == amendment.target);
                Some(amend_action(amending, amendment, target))
            })
            .collect();
        vec![Box::new(Amending { actions })]
    }
}

struct Amending {
    actions: Vec<Action>,
}

impl ViewController for Amending {
    fn id(&self) -> &str {
        "AmendmentActions"
    }

    fn actions(&self) -> Vec<Action> {
        self.actions.clone()
    }
}
