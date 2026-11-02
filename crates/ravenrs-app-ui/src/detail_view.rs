//! One record: a form laid out as the model says.

use std::rc::Rc;

use ravenrs_app::{ModelNode, ViewInfo};
use ravenrs_widgets::{AnyView, BuildCx, FormField, FormGroup, FormLayout, NodeId, Text, View};

use crate::editors::EditorRequest;
use crate::session::{FormState, Session};

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

    fn editor(&self, form: &Rc<FormState>, node: &ModelNode, field: &str) -> Option<AnyView> {
        let info = form.entity.field_named(field)?;
        let item = node.at(&["Items", field]);
        let caption = item.and_then(|n| n.text("Caption")).unwrap_or(&info.caption).to_owned();
        let read_only = item.and_then(|n| n.flag("ReadOnly")).unwrap_or(info.read_only);
        let value = match form.fields.get(field) {
            Some(value) => value.clone(),
            None => {
                // A calculated field follows the others.
                let mut current = form.current();
                form.entity.compute(&mut current, &ravenrs_criteria::Environment::new());
                ravenrs_reactive::State::new(current.get(field))
            }
        };
        let request = EditorRequest {
            field: info.clone(),
            value,
            caption: caption.clone(),
            read_only: read_only || (info.is_key && !form.is_new),
            session: self.session.clone(),
        };
        let name = item.and_then(|n| n.text("Editor")).unwrap_or("");
        let editor = self.session.editors().editor(name, &request);
        // A check box says its own name.
        if matches!(info.kind, ravenrs_app::FieldKind::Boolean) {
            return Some(editor);
        }
        Some(Box::new(FormField::new(caption, editor)))
    }

    fn group(&self, form: &Rc<FormState>, node: &ModelNode, source: &ModelNode) -> FormGroup {
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
            if let Some(editor) = self.editor(form, node, field) {
                group = if tall { group.wide(editor) } else { group.field(editor) };
            }
        }
        group
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
        let mut layout = FormLayout::new();
        if let Some(arrangement) = node.get("Layout") {
            for part in arrangement.in_order() {
                if part.text("Kind") == Some("Tabs") {
                    for tab in part.in_order() {
                        let title = tab.text("Caption").unwrap_or(&tab.id).to_owned();
                        layout = layout.tab(title, FormLayout::new().group(self.group(&form, node, tab)));
                    }
                } else {
                    layout = layout.group(self.group(&form, node, part));
                }
            }
        }
        cx.child(&layout)
    }
}
