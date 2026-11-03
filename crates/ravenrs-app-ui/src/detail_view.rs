//! One record: a form laid out as the model says.

use std::rc::Rc;

use ravenrs_app::{
    ActionCategory, AppearanceTarget, ModelNode, Object, RuleKind, Severity, ViewInfo, Violation,
};
use ravenrs_layout::Align;
use ravenrs_widgets::{
    Alert, AnyView, BuildCx, FormField, FormGroup, FormLayout, NodeId, Text, Tone, VStack, View,
};

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
        named_around: bool,
    ) -> Option<AnyView> {
        let info = form.entity.field_named(field)?;
        let item = node.at(&["Items", field]);
        let caption = item.and_then(|n| n.text("Caption")).unwrap_or(&info.caption).to_owned();
        let caption =
            self.session.form_caption().and_then(|words| words(field, &state.record)).unwrap_or(caption);
        let look = form.entity.appearance_of(AppearanceTarget::Item, field, &self.view, Some(&state.record));
        let around =
            form.entity.appearance_of(AppearanceTarget::Layout, field, &self.view, Some(&state.record));
        if look.visible == Some(false) || around.visible == Some(false) {
            return None;
        }
        if let ravenrs_app::FieldKind::Collection(target) = &info.kind {
            if let Some(source) = self.session.collection_source(&form.entity.name, field) {
                let record = form.current();
                let list = crate::nested::NestedList::given(
                    self.session.clone(),
                    target,
                    Rc::new(move || source(&record)),
                );
                return Some(if named_around {
                    Box::new(list) as AnyView
                } else {
                    Box::new(FormField::new(caption, list))
                });
            }
            let key = form.entity.key_field().map(|key| form.original.get(&key.name));
            let list = crate::nested::NestedList::new(
                self.session.clone(),
                &form.entity,
                (!form.is_new).then_some(key).flatten(),
                target,
                info.back_reference.as_deref(),
                info.lookup_filter.as_deref(),
            );
            return Some(if named_around {
                Box::new(list) as AnyView
            } else {
                Box::new(FormField::new(caption, list))
            });
        }
        let mut read_only = item.and_then(|n| n.flag("ReadOnly")).unwrap_or(info.read_only);
        if let Some(security) = self.session.frame().security() {
            let (name, record) = (&form.entity.name, Some(&state.record));
            let (can_read, can_write) = security.with(|p| {
                (
                    p.can_field(ravenrs_app::Operation::Read, name, field, record),
                    p.can_field(ravenrs_app::Operation::Write, name, field, record),
                )
            });
            if !can_read && !(form.is_new && can_write) {
                let shown = Text::new(self.session.words().protected_value.clone());
                return Some(Box::new(FormField::new(caption, shown)));
            }
            read_only |= !can_write;
        }
        let value = match form.fields.get(field) {
            Some(value) => value.clone(),
            None => {
                // A calculated field follows the others.
                let mut current = state.record.clone();
                form.entity.compute(&mut current, &ravenrs_app::environment());
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
            let (error, note) = (state.error_of(field), state.note_of(field));
            if error.is_none() && note.is_none() {
                return Some(editor);
            }
            let (note, tone) = note.map_or((None, Tone::Neutral), |(note, tone)| (Some(note), tone));
            return Some(Box::new(FormField::new(caption, editor).error_text(error).note_text(note, tone)));
        }
        let required = form
            .entity
            .rules
            .iter()
            .any(|rule| rule.field.as_deref() == Some(field) && matches!(rule.kind, RuleKind::Required));
        let mut wrapped = FormField::new(caption, editor).error_text(state.error_of(field));
        if let Some((note, tone)) = state.note_of(field) {
            wrapped = wrapped.note_text(Some(note), tone);
        }
        if let Some(hint) = &info.hint {
            wrapped = wrapped.hint(hint.clone());
        }
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
        in_tab: bool,
    ) -> FormGroup {
        let columns = source.integer("Columns").unwrap_or(1).max(1) as usize;
        // The tab already says it.
        let title = source.text("Caption").filter(|_| !in_tab);
        let mut group = match title {
            Some(title) => FormGroup::titled(title),
            None => FormGroup::new(),
        }
        .columns(columns);
        for item in source.in_order() {
            if item.text("Kind") == Some("ActionContainer") {
                let name = item.text("Category").unwrap_or(&item.id).to_owned();
                group = group
                    .wide(crate::ActionContainer::new(self.session.clone(), [ActionCategory::InForm(name)]));
                continue;
            }
            let field = item.text("Field").unwrap_or(&item.id);
            let tall = item.flag("Wide") == Some(true)
                || node.at(&["Items", field]).and_then(|n| n.flag("Wide")) == Some(true)
                || form.entity.field_named(field).is_some_and(|f| {
                    matches!(
                        f.kind,
                        ravenrs_app::FieldKind::LongText
                            | ravenrs_app::FieldKind::Image
                            | ravenrs_app::FieldKind::Collection(_)
                    )
                });
            let named_around =
                in_tab || form.entity.field_named(field).is_some_and(|f| title == Some(f.caption.as_str()));
            if let Some(editor) = self.editor(form, node, field, state, named_around) {
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

fn tone_of(severity: Severity) -> Tone {
    if severity == Severity::Warning { Tone::Warning } else { Tone::Accent }
}

/// What the rules see in this build of a form.
struct FormLook {
    record: Object,
    violations: Vec<Violation>,
}

impl FormLook {
    fn error_of(&self, field: &str) -> Option<String> {
        self.violations
            .iter()
            .find(|v| v.is_error() && v.field.as_deref() == Some(field))
            .map(|v| v.message.clone())
    }

    /// What the rules only want known about a field.
    fn note_of(&self, field: &str) -> Option<(String, Tone)> {
        self.violations
            .iter()
            .find(|v| !v.is_error() && v.field.as_deref() == Some(field))
            .map(|v| (v.message.clone(), tone_of(v.severity)))
    }
}

impl View for DetailView {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let session = &self.session;
        // The model can change under a form, as with another language.
        session.revision();
        let model = session.model();
        let Some(node) = model.at(&["Views", &self.view.id]) else {
            return cx.child(&Text::new(self.view.id.clone()));
        };
        let Some(form) = session.form() else {
            return cx.empty();
        };
        let state = FormLook { record: form.watched(), violations: session.form_violations() };
        let lists_all = session.lists_field_errors();
        let messages: Vec<String> = state
            .violations
            .iter()
            .filter(|v| v.is_error() && (lists_all || v.field.is_none()))
            .map(|v| v.message.clone())
            .collect();
        let notes: Vec<&Violation> =
            state.violations.iter().filter(|v| !v.is_error() && v.field.is_none()).collect();
        let note = (!notes.is_empty()).then(|| {
            let text: Vec<&str> = notes.iter().map(|v| v.message.as_str()).collect();
            let warns = notes.iter().any(|v| v.severity == Severity::Warning);
            Alert::message(text.join("\n")).tone(if warns { Tone::Warning } else { Tone::Accent })
        });
        let mut layout = FormLayout::new().errors(messages);
        if let Some(arrangement) = node.get("Layout") {
            for part in arrangement.in_order() {
                if !self.shows(&form, part, &state) {
                    continue;
                }
                if part.text("Kind") == Some("Tabs") {
                    for tab in part.in_order().into_iter().filter(|tab| self.shows(&form, tab, &state)) {
                        let title = tab.text("Caption").unwrap_or(&tab.id).to_owned();
                        layout = layout
                            .tab(title, FormLayout::new().group(self.group(&form, node, tab, &state, true)));
                    }
                } else {
                    layout = layout.group(self.group(&form, node, part, &state, false));
                }
            }
        }
        match note {
            Some(note) => cx.child(&VStack::new((note, layout)).spacing(12.0).align(Align::Stretch)),
            None => cx.child(&layout),
        }
    }
}
