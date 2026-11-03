//! How things look and behave by what a record holds.

use std::fmt;
use std::rc::Rc;

use ravenrs_criteria::{Criteria, Environment, Scope};

use crate::frame::{ViewInfo, ViewKind};
use crate::{EntityInfo, ModelError, Object};

/// A color with a meaning, which every theme has its own version of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Tint {
    /// Something wrong.
    Danger,
    /// Something to watch.
    Warning,
    /// Something right.
    Success,
    /// What stands out.
    Accent,
    /// What steps back.
    Muted,
}

/// What a rule changes the look of.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AppearanceTarget {
    /// A field: a cell of a list, an editor on a form.
    #[default]
    Item,
    /// A group or a tab of a form.
    Layout,
    /// An action.
    Action,
}

/// Which views a rule applies in.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum AppearanceContext {
    /// Every view.
    #[default]
    Any,
    /// Only lists.
    List,
    /// Only forms.
    Detail,
    /// Only the view with this id.
    View(String),
}

impl AppearanceContext {
    fn covers(&self, view: &ViewInfo) -> bool {
        match self {
            Self::Any => true,
            Self::List => view.kind == ViewKind::List,
            Self::Detail => view.kind == ViewKind::Detail,
            Self::View(id) => *id == view.id,
        }
    }
}

type Condition = Rc<dyn Fn(&Object) -> bool>;

/// Changes how things look or behave, for records that meet a condition. It is about looks:
/// it does not protect data, which is what permissions are for.
#[derive(Clone)]
#[non_exhaustive]
pub struct AppearanceRule {
    /// What it is known by.
    pub id: String,
    /// The kind of thing it changes.
    pub target: AppearanceTarget,
    /// The names of those things, where `*` stands for any text. A list that starts with a
    /// lone `*` means all but the names after it.
    pub names: Vec<String>,
    /// Where it applies.
    pub context: AppearanceContext,
    /// The expression a record has to meet. With none, and no function, it always applies.
    pub criteria: Option<String>,
    /// Whichever is higher wins, for what both change.
    pub priority: i32,
    /// The color of the text.
    pub font: Option<Tint>,
    /// The color behind it.
    pub back: Option<Tint>,
    /// Heavier text.
    pub bold: Option<bool>,
    /// Whether it can be used.
    pub enabled: Option<bool>,
    /// Whether it shows.
    pub visible: Option<bool>,
    function: Option<Condition>,
}

impl fmt::Debug for AppearanceRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppearanceRule").field("id", &self.id).finish_non_exhaustive()
    }
}

impl PartialEq for AppearanceRule {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl AppearanceRule {
    /// A rule about fields with these names, as `*` or `open*`.
    pub fn new(id: impl Into<String>, names: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            id: id.into(),
            target: AppearanceTarget::Item,
            names: names.into_iter().map(Into::into).collect(),
            context: AppearanceContext::Any,
            criteria: None,
            priority: 0,
            font: None,
            back: None,
            bold: None,
            enabled: None,
            visible: None,
            function: None,
        }
    }

    /// About this kind of thing.
    pub fn target(mut self, target: AppearanceTarget) -> Self {
        self.target = target;
        self
    }

    /// Only in these views.
    pub fn context(mut self, context: AppearanceContext) -> Self {
        self.context = context;
        self
    }

    /// For records that meet this expression.
    pub fn criteria(mut self, expression: impl Into<String>) -> Self {
        self.criteria = Some(expression.into());
        self
    }

    /// For records this says yes to.
    pub fn when(mut self, condition: impl Fn(&Object) -> bool + 'static) -> Self {
        self.function = Some(Rc::new(condition));
        self
    }

    /// Whichever is higher wins.
    pub fn priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// The color of the text.
    pub fn font(mut self, tint: Tint) -> Self {
        self.font = Some(tint);
        self
    }

    /// The color behind it.
    pub fn back(mut self, tint: Tint) -> Self {
        self.back = Some(tint);
        self
    }

    /// Heavier text.
    pub fn bold(mut self) -> Self {
        self.bold = Some(true);
        self
    }

    /// Whether it can be used.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = Some(enabled);
        self
    }

    /// Whether it shows.
    pub fn visible(mut self, visible: bool) -> Self {
        self.visible = Some(visible);
        self
    }

    pub(crate) fn check(&self, entity: &EntityInfo) -> Result<(), ModelError> {
        if let Some(text) = &self.criteria {
            Criteria::parse(text)
                .map_err(|e| ModelError(format!("{}: the rule {}: {e}", entity.name, self.id)))?;
        }
        Ok(())
    }

    fn names_cover(&self, name: &str) -> bool {
        match self.names.split_first() {
            Some((first, excluded)) if first == "*" => {
                !excluded.iter().any(|pattern| matches_pattern(pattern, name))
            }
            _ => self.names.iter().any(|pattern| matches_pattern(pattern, name)),
        }
    }

    /// Whether it applies to this record. With no record, as for a whole column, only a
    /// rule with no condition does.
    fn applies(&self, object: Option<&Object>, environment: &Environment) -> bool {
        let unconditional = self.criteria.is_none() && self.function.is_none();
        let Some(object) = object else { return unconditional };
        self.criteria.as_deref().is_none_or(|text| {
            Criteria::parse(text).is_ok_and(|parsed| Scope::new(environment).row(object).matches(&parsed))
        }) && self.function.as_ref().is_none_or(|condition| condition(object))
    }
}

fn matches_pattern(pattern: &str, name: &str) -> bool {
    let Some((start, rest)) = pattern.split_once('*') else { return pattern == name };
    if !name.starts_with(start) {
        return false;
    }
    let mut left = &name[start.len()..];
    let parts: Vec<&str> = rest.split('*').collect();
    for (i, part) in parts.iter().enumerate() {
        if i + 1 == parts.len() {
            return left.ends_with(part);
        }
        match left.find(part) {
            Some(at) => left = &left[at + part.len()..],
            None => return false,
        }
    }
    true
}

/// What the rules that apply come to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Appearance {
    /// The color of the text.
    pub font: Option<Tint>,
    /// The color behind it.
    pub back: Option<Tint>,
    /// Heavier text.
    pub bold: bool,
    /// Whether it can be used, if a rule said.
    pub enabled: Option<bool>,
    /// Whether it shows, if a rule said.
    pub visible: Option<bool>,
}

impl EntityInfo {
    /// What this entity's rules say about a thing in a view, for a record.
    pub fn appearance_of(
        &self,
        target: AppearanceTarget,
        name: &str,
        view: &ViewInfo,
        object: Option<&Object>,
    ) -> Appearance {
        let environment = crate::environment();
        let mut rules: Vec<&AppearanceRule> = self
            .appearance
            .iter()
            .filter(|rule| rule.target == target && rule.context.covers(view) && rule.names_cover(name))
            .filter(|rule| rule.applies(object, &environment))
            .collect();
        rules.sort_by_key(|rule| rule.priority);
        let mut result = Appearance::default();
        for rule in rules {
            result.font = rule.font.or(result.font);
            result.back = rule.back.or(result.back);
            result.bold = rule.bold.unwrap_or(result.bold);
            result.enabled = rule.enabled.or(result.enabled);
            result.visible = rule.visible.or(result.visible);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stars_stand_for_any_text() {
        for (pattern, name, fits) in [
            ("*", "anything", true),
            ("name", "name", true),
            ("name", "names", false),
            ("open*", "opening", true),
            ("open*", "reopen", false),
            ("*ing", "opening", true),
            ("*ing", "ingot", false),
            ("o*n*g", "opening", true),
            ("o*x*g", "opening", false),
        ] {
            assert_eq!(matches_pattern(pattern, name), fits, "{pattern} {name}");
        }
    }
}
