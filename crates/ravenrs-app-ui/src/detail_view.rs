//! One record: a form laid out as the model says.

use std::rc::Rc;

use ravenrs_app::{AppearanceTarget, ModelNode, Object, RuleKind, ViewInfo, Violation};
use ravenrs_widgets::{AnyView, BuildCx, FormField, FormGroup, FormLayout, NodeId, Text, View};

use crate::editors::EditorRequest;
use crate::session::{FormState, Session};
use crate::values::semantic_of;

/// The form of the record being edited. What each field uses comes from the model's `Editor`.
#[derive(Debug)]
pub struct DetailView {
    session: Session,
    view: ViewInfo,
}

impl DetailView {
    /// The form of this view in the model.
    pub fn new(session: Session, view: ViewInfo) -> Self {
        Self { session, view }
    }

    fn editor(
        &self,
        form: &Rc<FormState>,
        node: &ModelNode,
        field: &str,
        state: &FormLook,
    ) -> Option<AnyView> {
        let info = form.entity.field_named(field)?;
        let item = node.at(&["Items", field]);
        let caption = item.and_then(|n| n.text("Caption")).unwrap_or(&info.caption).to_owned();
        let look = form.entity.appearance_of(AppearanceTarget::Item, field, &self.view, Some(&state.record));
        let around =
            form.entity.appearance_of(AppearanceTarget::Layout, field, &self.view, Some(&state.record));
        if look.visible == Some(false) || around.visible == Some(false) {
            return None;
        }
        let read_only = item.and_then(|n| n.flag("ReadOnly")).unwrap_or(info.read_only);
        let value = match form.fields.get(field) {
            Some(value) => value.clone(),
            None => {
                // A calculated field follows the others.
                let mut current = state.record.clone();
                form.entity.compute(&mut current, &ravenrs_criteria::Environment::new());
                ravenrs_reactive::State::new(current.get(field))
            }
        };
        let request = EditorRequest {
            field: info.clone(),
            value,
            caption: caption.clone(),
            read_only: read_only || (info.is_key && !form.is_new) || look.enabled == Some(false),
            session: self.session.clone(),
        };
        let name = item.and_then(|n| n.text("Editor")).unwrap_or("");
        let editor = self.session.editors().editor(name, &request);
        // A check box says its own name.
        if matches!(info.kind, ravenrs_app::FieldKind::Boolean) {
            let error = state.error_of(field);
            return Some(match error {
                Some(error) => Box::new(FormField::new(caption, editor).error_text(Some(error))),
                None => editor,
            });
        }
        let required = form
            .entity
            .rules
            .iter()
            .any(|rule| rule.field.as_deref() == Some(field) && matches!(rule.kind, RuleKind::Required));
        let mut wrapped = FormField::new(caption, editor).error_text(state.error_of(field));
        if required {
            wrapped = wrapped.required();
        }
        if let Some(tint) = look.font.or(around.font) {
            wrapped = wrapped.label_ink(semantic_of(tint));
        }
        if let Some(tint) = look.back.or(around.back) {
            wrapped = wrapped.tint(semantic_of(tint));
        }
        Some(Box::new(wrapped))
    }

    fn group(
        &self,
        form: &Rc<FormState>,
        node: &ModelNode,
        source: &ModelNode,
        state: &FormLook,
    ) -> FormGroup {
        let columns = source.integer("Columns").unwrap_or(1).max(1) as usize;
        let mut group = match source.text("Caption") {
            Some(title) => FormGroup::titled(title),
            None => FormGroup::new(),
        }
        .columns(columns);
        for item in source.in_order() {
            let field = item.text("Field").unwrap_or(&item.id);
            let tall = form.entity.field_named(field).is_some_and(|f| {
                matches!(f.kind, ravenrs_app::FieldKind::LongText | ravenrs_app::FieldKind::Image)
            });
            if let Some(editor) = self.editor(form, node, field, state) {
                group = if tall { group.wide(editor) } else { group.field(editor) };
            }
        }
        group
    }

    fn shows(&self, form: &FormState, part: &ModelNode, state: &FormLook) -> bool {
        form.entity.appearance_of(AppearanceTarget::Layout, &part.id, &self.view, Some(&state.record)).visible
            != Some(false)
    }
}

/// What the rules see in this build of a form.
struct FormLook {
    record: Object,
    violations: Vec<Violation>,
}

impl FormLook {
    fn error_of(&self, field: &str) -> Option<String> {
        self.violations.iter().find(|v| v.field.as_deref() == Some(field)).map(|v| v.message.clone())
    }
}

impl View for DetailView {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let session = &self.session;
        let model = session.model();
        let Some(node) = model.at(&["Views", &self.view.id]) else {
            return cx.child(&Text::new(self.view.id.clone()));
        };
        let Some(form) = session.form() else {
            return cx.empty();
        };
        let state = FormLook { record: form.watched(), violations: session.form_violations() };
        let messages: Vec<String> = state.violations.iter().map(|v| v.message.clone()).collect();
        let mut layout = FormLayout::new().errors(messages);
        if let Some(arrangement) = node.get("Layout") {
            for part in arrangement.in_order() {
                if !self.shows(&form, part, &state) {
                    continue;
                }
                if part.text("Kind") == Some("Tabs") {
                    for tab in part.in_order().into_iter().filter(|tab| self.shows(&form, tab, &state)) {
                        let title = tab.text("Caption").unwrap_or(&tab.id).to_owned();
                        layout =
                            layout.tab(title, FormLayout::new().group(self.group(&form, node, tab, &state)));
                    }
                } else {
                    layout = layout.group(self.group(&form, node, part, &state));
                }
            }
        }
        cx.child(&layout)
    }
}
