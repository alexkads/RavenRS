//! How things look and behave by what a record holds.

use std::fmt;
use std::rc::Rc;

use ravenrs_criteria::{Criteria, Environment, Scope};

pub use crate::entity::appearance_types::{AppearanceContext, AppearanceRule, AppearanceTarget, Tint};
use crate::frame::{ViewInfo, ViewKind};
use crate::{EntityInfo, ModelError, Object};

#[cfg_attr(feature = "live", ravenrs_live::expose)]
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

#[cfg_attr(feature = "live", ravenrs_live::expose)]
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
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
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

#[cfg_attr(feature = "live", ravenrs_live::expose)]
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
