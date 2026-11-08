//! A form in a window, for a record an application keeps in its own way.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use ravenrs_app::{Application, EntityInfo, ModelError, Object, ObjectSpace, Value, ViewInfo};
use ravenrs_layout::Align;
use ravenrs_reactive::{Binding, State};
use ravenrs_widgets::{
    AnyView, BuildCx, Button, ButtonStyle, Dialog, IconName, Keyed, NodeId, ScrollView, VStack, View,
};

use crate::session::{FormCaption, Session, SpaceFactory, Words};
use crate::{DetailView, EditorRegistry, SystemModule};

/// What the window of a form says.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct FormText {
    /// Its title.
    pub title: String,
    /// What it says under the title.
    pub description: Option<String>,
    /// The button that keeps the record.
    pub save: String,
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl FormText {
    /// A window with this title and this button.
    pub fn new(title: impl Into<String>, save: impl Into<String>) -> Self {
        Self { title: title.into(), description: None, save: save.into() }
    }

    /// What it says under the title.
    pub fn description(mut self, text: impl Into<String>) -> Self {
        self.description = Some(text.into());
        self
    }
}

type SaveHandler = Rc<dyn Fn(&Object, bool) -> bool>;

struct FormInner {
    session: Session,
    entity: EntityInfo,
    view: ViewInfo,
    open: State<bool>,
    text: State<FormText>,
    on_save: RefCell<Option<SaveHandler>>,
    width: Cell<f32>,
    height: Cell<Option<f32>>,
}

/// A form of an entity in a window. It checks its rules and hands the record over: keeping it is
/// up to whoever asked, so a record with rules of its own keeps going through them. A handle.
#[derive(Clone)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct EntityForm(Rc<FormInner>);

impl std::fmt::Debug for EntityForm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EntityForm").field("entity", &self.0.entity.name).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl EntityForm {
    /// A form of this entity, among these. References find their records in `space`.
    pub fn new<S: ObjectSpace + 'static>(
        entity: &str,
        entities: Vec<EntityInfo>,
        space: impl Fn() -> S + 'static,
        words: Words,
        editors: EditorRegistry,
    ) -> Result<Self, ModelError> {
        let application = entities
            .iter()
            .cloned()
            .fold(
                Application::builder(entity).module(SystemModule::with_words(words.clone())),
                ravenrs_app::ApplicationBuilder::entity,
            )
            .build()?;
        let info = entities
            .iter()
            .find(|found| found.name == entity)
            .cloned()
            .ok_or_else(|| ModelError(format!("there is no entity {entity}")))?;
        let view = ViewInfo::from_model(&application.model(), &format!("{entity}_DetailView"))?;
        let factory: SpaceFactory =
            Rc::new(move || Rc::new(RefCell::new(space())) as Rc<RefCell<dyn ObjectSpace>>);
        let session = Session::with_editors(application, factory, words, editors);
        session.set_lists_field_errors(false);
        Ok(Self(Rc::new(FormInner {
            session,
            entity: info,
            view,
            open: State::new(false),
            text: State::new(FormText::new("", "")),
            on_save: RefCell::default(),
            width: Cell::new(440.0),
            height: Cell::new(None),
        })))
    }

    /// Takes a record that met its rules, and whether it is new; false leaves the form open.
    pub fn on_save(self, handler: impl Fn(&Object, bool) -> bool + 'static) -> Self {
        *self.0.on_save.borrow_mut() = Some(Rc::new(handler));
        self
    }

    /// The width of the window. 440 by default.
    pub fn width(self, width: f32) -> Self {
        self.0.width.set(width);
        self
    }

    /// Scrolls its fields inside this height.
    pub fn scrolling_height(self, height: f32) -> Self {
        self.0.height.set(Some(height));
        self
    }

    /// Says what a field is called when that depends on the record.
    pub fn captions(self, caption: FormCaption) -> Self {
        self.0.session.set_form_caption(caption);
        self
    }

    /// What its screens share.
    pub fn session(&self) -> &Session {
        &self.0.session
    }

    /// Opens on a new record, filled in by `fill`.
    pub fn start_new(&self, text: FormText, fill: impl FnOnce(&mut Object)) {
        let mut object = self.0.entity.new_object();
        fill(&mut object);
        self.open_on(text, object, true);
    }

    /// Opens on the record with this key, changed by `fill` first. Nothing happens if it is gone.
    pub fn start_edit(&self, text: FormText, key: &Value, fill: impl FnOnce(&mut Object)) {
        let found = self.0.session.frame().space().borrow().find(&self.0.entity.name, key);
        if let Ok(Some(mut object)) = found {
            fill(&mut object);
            self.open_on(text, object, false);
        }
    }

    fn open_on(&self, text: FormText, object: Object, is_new: bool) {
        self.0.session.open_object(&self.0.entity, object, is_new);
        self.0.text.set(text);
        self.0.open.set(true);
    }

    /// Changes a field of the record on the form.
    pub fn set(&self, field: &str, value: impl Into<Value>) {
        self.0.session.set_form_value(field, value.into());
    }

    /// Whether it is open.
    pub fn is_open(&self) -> bool {
        *self.0.open.peek()
    }

    /// Closes it, keeping nothing.
    pub fn close(&self) {
        self.0.open.set(false);
    }

    /// Hands over the record if it meets its rules, and closes if it was taken.
    pub fn save(&self) {
        // A second click can arrive before the form is gone.
        if !self.is_open() {
            return;
        }
        let Some((object, is_new)) = self.0.session.check_form() else { return };
        let handler = self.0.on_save.borrow().clone();
        if handler.is_none_or(|handler| handler(&object, is_new)) {
            self.close();
        }
    }
}

impl View for EntityForm {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let inner = &self.0;
        if !inner.open.get() {
            return cx.empty();
        }
        let text = inner.text.get();
        let form = inner.session.form().map_or(0, |form| Rc::as_ptr(&form) as usize as u64);
        let fields = Keyed::new(form, DetailView::new(inner.session.clone(), inner.view.clone()));
        let content: AnyView = match inner.height.get() {
            Some(height) => Box::new(
                ScrollView::new((VStack::new((fields,)).align(Align::Stretch),))
                    .height(height)
                    .scrollbar_outside(14.0),
            ),
            None => Box::new(fields),
        };
        let open = inner.open.clone();
        let (read, write) = (open.clone(), open.clone());
        let binding = Binding::new(move || read.get(), move |value| write.set(value));
        let saving = self.clone();
        let mut dialog = Dialog::new(binding, text.title.clone())
            .width(inner.width.get())
            .content(content)
            .action(
                Button::new(inner.session.words().cancel.clone())
                    .style(ButtonStyle::Secondary)
                    .on_click(move || open.set(false)),
            )
            .action(Button::new(text.save.clone()).icon(IconName::Check).on_click(move || saving.save()));
        if let Some(description) = text.description.clone() {
            dialog = dialog.description(description);
        }
        cx.child(&dialog)
    }
}
