//! What screen readers call the fields and the actions of an application, from the model.
//!
//! The caption is the name, unless the model says otherwise. A field says it with
//! [`FieldInfo::accessible_name`] and [`FieldInfo::accessible_description`], which the model
//! copies to the item of its form as `AccessibleName` and `AccessibleDescription`. There a
//! translation or an administrator changes them, as they do the caption. An action says it with
//! [`Action::accessible_name`] and [`Action::accessible_description`].

use ravenrs_app::{Action, FieldInfo, ModelNode};

/// The name and the description screen readers get for a field or an action.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct AccessibleText {
    /// What it is called.
    pub name: String,
    /// What is said after the name. With none, the control keeps what it has, such as the hint
    /// of a field.
    pub description: Option<String>,
    /// The name is not the caption on the screen.
    pub renamed: bool,
}

impl AccessibleText {
    /// For a field whose form item is `item` and which the screen calls `caption`. The model's
    /// item comes first, then the declaration, then the caption.
    pub fn of_field(field: &FieldInfo, item: Option<&ModelNode>, caption: &str) -> Self {
        let said = |name: &str, declared: &Option<String>| {
            item.and_then(|item| item.text(name))
                .map(str::to_owned)
                .or_else(|| declared.clone())
                .filter(|text| !text.trim().is_empty())
        };
        let named = said("AccessibleName", &field.accessible_name);
        Self {
            renamed: named.as_deref().is_some_and(|name| name != caption),
            name: named.unwrap_or_else(|| caption.to_owned()),
            description: said("AccessibleDescription", &field.accessible_description),
        }
    }

    /// For an action: its own words, or its caption.
    pub fn of_action(action: &Action) -> Self {
        let named = action.accessible_name_text().filter(|text| !text.trim().is_empty());
        Self {
            renamed: named.is_some_and(|name| name != action.caption()),
            name: named.unwrap_or(action.caption()).to_owned(),
            description: action.accessible_description_text().map(str::to_owned),
        }
    }

    /// Whether the control has to be told anything besides what it shows.
    pub fn says_more(&self) -> bool {
        self.renamed || self.description.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ravenrs_app::FieldKind;

    #[test]
    fn the_caption_is_the_name_unless_the_model_says_otherwise() {
        let field = FieldInfo::new("until", FieldKind::Date).caption("Até");
        let plain = AccessibleText::of_field(&field, None, "Até");
        assert_eq!((plain.name.as_str(), plain.renamed, plain.says_more()), ("Até", false, false));

        let field = field.accessible_name("Fim do período").accessible_description("O último dia");
        let declared = AccessibleText::of_field(&field, None, "Até");
        assert_eq!(declared.name, "Fim do período");
        assert_eq!(declared.description.as_deref(), Some("O último dia"));
        assert!(declared.renamed);

        let item = ModelNode::new("until").with("AccessibleName", "End of the period");
        let translated = AccessibleText::of_field(&field, Some(&item), "Until");
        assert_eq!(translated.name, "End of the period", "the model's item comes first");
    }

    #[test]
    fn an_action_is_its_caption_unless_it_says_otherwise() {
        let next = Action::simple("Next", "›");
        assert_eq!(AccessibleText::of_action(&next).name, "›");
        let next =
            next.accessible_name("Próximo registro").accessible_description("Abre o seguinte da lista");
        let text = AccessibleText::of_action(&next);
        assert_eq!(text.name, "Próximo registro");
        assert!(text.renamed && text.says_more());
    }
}
