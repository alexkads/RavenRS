//! What the screens of a running application share.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

use ravenrs_app::{
    Action, ActionContext, ActionInput, ActionResult, Application, CONTEXT_DELETE, CONTEXT_SAVE, EntityInfo,
    Frame, FrameContext, ModelNode, Object, ObjectSpace, Query, RuleWords, Value, ViewInfo, ViewKind,
    Violation, validate,
};
use ravenrs_reactive::State;
use ravenrs_widgets::{ConfirmDialog, Date, FilePicker};

use crate::editors::EditorRegistry;
use crate::values::key_text;

/// The words the generated screens use.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Words {
    /// Creates a record.
    pub new: String,
    /// Removes what is selected.
    pub delete: String,
    /// Keeps the changes.
    pub save: String,
    /// Keeps the changes and leaves.
    pub save_and_close: String,
    /// Keeps the changes and starts another.
    pub save_and_new: String,
    /// Throws away the changes and leaves.
    pub cancel: String,
    /// The record before.
    pub previous: String,
    /// The record after.
    pub next: String,
    /// Reads again.
    pub refresh: String,
    /// Goes to the screen before.
    pub back: String,
    /// The question before removing.
    pub delete_question: String,
    /// What the removal does.
    pub delete_detail: String,
    /// The question before losing changes.
    pub discard_question: String,
    /// What losing changes does.
    pub discard_detail: String,
    /// Agrees to lose them.
    pub discard: String,
    /// A record being created.
    pub new_record: String,
    /// The menu of the ways to see a list.
    pub variant: String,
    /// Puts an existing record on a list inside a form.
    pub link: String,
    /// Takes a record off a list inside a form, without removing it.
    pub unlink: String,
    /// Where a list inside a form would be, while its record is not kept yet.
    pub save_first: String,
    /// What fails a rule.
    pub rules: RuleWords,
    /// In front of money.
    pub money_prefix: String,
    /// Between thousands.
    pub thousands: char,
    /// Before decimals.
    pub decimal: char,
}

impl Default for Words {
    fn default() -> Self {
        Self {
            new: "New".into(),
            delete: "Delete".into(),
            save: "Save".into(),
            save_and_close: "Save and close".into(),
            save_and_new: "Save and new".into(),
            cancel: "Cancel".into(),
            previous: "Previous".into(),
            next: "Next".into(),
            refresh: "Refresh".into(),
            back: "Back".into(),
            delete_question: "Delete?".into(),
            delete_detail: "This cannot be undone.".into(),
            discard_question: "Discard the changes?".into(),
            discard_detail: "What you changed here was not saved.".into(),
            discard: "Discard".into(),
            new_record: "New".into(),
            variant: "View".into(),
            link: "Link".into(),
            unlink: "Unlink".into(),
            save_first: "Save this record to add to this list.".into(),
            rules: RuleWords::default(),
            money_prefix: "$ ".into(),
            thousands: ',',
            decimal: '.',
        }
    }
}

impl Words {
    /// In Brazilian Portuguese.
    pub fn pt_br() -> Self {
        Self {
            new: "Novo".into(),
            delete: "Excluir".into(),
            save: "Salvar".into(),
            save_and_close: "Salvar e fechar".into(),
            save_and_new: "Salvar e novo".into(),
            cancel: "Cancelar".into(),
            previous: "Anterior".into(),
            next: "Próximo".into(),
            refresh: "Atualizar".into(),
            back: "Voltar".into(),
            delete_question: "Excluir?".into(),
            delete_detail: "Não será possível desfazer.".into(),
            discard_question: "Descartar as alterações?".into(),
            discard_detail: "O que você mudou aqui não foi salvo.".into(),
            discard: "Descartar".into(),
            new_record: "Novo".into(),
            variant: "Visão".into(),
            link: "Vincular".into(),
            unlink: "Desvincular".into(),
            save_first: "Salve este registro para acrescentar a esta lista.".into(),
            rules: RuleWords::pt_br(),
            money_prefix: "R$ ".into(),
            thousands: '.',
            decimal: ',',
        }
    }
}

/// The record a form is editing: what it was, and one state per field.
#[derive(Debug)]
pub(crate) struct FormState {
    pub entity: EntityInfo,
    pub original: Object,
    pub is_new: bool,
    pub fields: BTreeMap<String, State<Value>>,
}

impl FormState {
    fn new(entity: EntityInfo, original: Object, is_new: bool) -> Self {
        let fields = entity
            .fields
            .iter()
            .filter(|f| f.calculated.is_none())
            .map(|f| (f.name.clone(), State::new(original.get(&f.name))))
            .collect();
        Self { entity, original, is_new, fields }
    }

    /// The record with what the fields hold now, without reading them for the build.
    pub fn current(&self) -> Object {
        let mut object = self.original.clone();
        for (name, value) in &self.fields {
            object.set(name, value.peek().clone());
        }
        object
    }

    /// The same, reading the fields for the build, so what depends on them follows.
    pub fn watched(&self) -> Object {
        let mut object = self.original.clone();
        for (name, value) in &self.fields {
            object.set(name, value.get());
        }
        object
    }

    pub fn is_dirty(&self) -> bool {
        self.is_new || self.fields.iter().any(|(name, value)| *value.peek() != self.original.get(name))
    }
}

/// Takes a file, by its name and bytes.
pub(crate) type FileHandler = Rc<dyn Fn(String, Vec<u8>)>;

/// Takes an address to open.
pub(crate) type UrlHandler = Rc<dyn Fn(String)>;

/// Makes a unit of work.
pub(crate) type SpaceFactory = Rc<dyn Fn() -> Rc<RefCell<dyn ObjectSpace>>>;

/// Says what a field of a form is called, for a field and the record as it stands, when that
/// depends on the record; nothing means the usual.
pub type FormCaption = Rc<dyn Fn(&str, &Object) -> Option<String>>;

/// Makes a control an application registered.
pub(crate) type ControlFactory = Rc<dyn Fn() -> ravenrs_widgets::AnyView>;

/// Something shown in a window over the screens, with its title.
pub(crate) type Presented = (String, ControlFactory);

struct SessionInner {
    application: Rc<RefCell<Application>>,
    new_space: SpaceFactory,
    frame: Frame,
    words: Words,
    editors: EditorRegistry,
    /// Goes up whenever records may have changed, so lists read again.
    revision: State<u64>,
    form: State<Option<Rc<FormState>>>,
    /// A save failed its rules, so the form says what is wrong as people fix it.
    checking: State<bool>,
    /// Where it came from, for going back.
    history: State<Vec<String>>,
    error: State<Option<String>>,
    confirm: ConfirmDialog,
    on_file: Rc<RefCell<Option<FileHandler>>>,
    on_url: Rc<RefCell<Option<UrlHandler>>>,
    controls: Rc<RefCell<BTreeMap<String, ControlFactory>>>,
    presented: State<Option<Presented>>,
    form_caption: RefCell<Option<FormCaption>>,
    /// Whether the form repeats, above it, what it says on each field.
    lists_field_errors: Cell<bool>,
    today: Cell<Date>,
    /// The keys of the list a form came from, for moving between records.
    siblings: RefCell<Vec<Value>>,
}

/// What the screens of a running application share: the frame, the current form, and what
/// has to happen next. A handle.
#[derive(Clone)]
pub struct Session(Rc<SessionInner>);

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session").field("frame", &self.0.frame).finish_non_exhaustive()
    }
}

/// How a frame finds its session.
struct SessionLink(std::rc::Weak<SessionInner>);

impl Session {
    pub(crate) fn new(
        application: Application,
        new_space: SpaceFactory,
        words: Words,
        editors: EditorRegistry,
        today: Date,
    ) -> Self {
        let frame = application.new_frame(FrameContext::Main, new_space());
        Self::linked(SessionInner {
            application: Rc::new(RefCell::new(application)),
            new_space,
            frame,
            words,
            editors,
            revision: State::new(0),
            form: State::new(None),
            checking: State::new(false),
            history: State::new(Vec::new()),
            error: State::new(None),
            confirm: ConfirmDialog::new(),
            on_file: Rc::default(),
            on_url: Rc::default(),
            controls: Rc::default(),
            presented: State::new(None),
            form_caption: RefCell::default(),
            lists_field_errors: Cell::new(true),
            today: Cell::new(today),
            siblings: RefCell::default(),
        })
    }

    fn linked(inner: SessionInner) -> Self {
        let inner = Rc::new(inner);
        let frame = inner.frame.clone();
        frame.set_extension(Rc::new(SessionLink(Rc::downgrade(&inner))));
        frame.set_validator(|contexts, cx| {
            let Some(session) = Session::of(cx) else { return Ok(()) };
            let form = session.form();
            let records = match (&form, cx.selected.is_empty()) {
                (Some(form), true) => vec![form.current()],
                _ => cx.selected.clone(),
            };
            let mut messages = Vec::new();
            for record in &records {
                for context in contexts {
                    messages.extend(session.violations_of(record, context).into_iter().map(|v| v.message));
                }
            }
            if messages.is_empty() { Ok(()) } else { Err(messages.join("; ")) }
        });
        Self(inner)
    }

    /// A session of its own for a frame inside this one or over it: its own form and unit of
    /// work, the same application, and lists that follow what the others change.
    pub(crate) fn child(&self, context: FrameContext) -> Self {
        let parent = &self.0;
        Self::linked(SessionInner {
            application: parent.application.clone(),
            new_space: parent.new_space.clone(),
            frame: self.new_frame(context),
            words: parent.words.clone(),
            editors: parent.editors.clone(),
            revision: parent.revision.clone(),
            form: State::new(None),
            checking: State::new(false),
            history: State::new(Vec::new()),
            error: State::new(None),
            confirm: ConfirmDialog::new(),
            on_file: parent.on_file.clone(),
            on_url: parent.on_url.clone(),
            controls: parent.controls.clone(),
            presented: parent.presented.clone(),
            form_caption: RefCell::new(parent.form_caption.borrow().clone()),
            lists_field_errors: Cell::new(parent.lists_field_errors.get()),
            today: Cell::new(parent.today.get()),
            siblings: RefCell::default(),
        })
    }

    /// A session for lists and actions inside a screen an application made itself, with no window
    /// around them. Showing a view is up to it, with [`show_list`](Self::show_list).
    pub fn embedded<S: ObjectSpace + 'static>(
        application: Application,
        space: impl Fn() -> S + 'static,
        words: Words,
    ) -> Self {
        let factory: SpaceFactory =
            Rc::new(move || Rc::new(RefCell::new(space())) as Rc<RefCell<dyn ObjectSpace>>);
        Self::standalone(application, factory, words)
    }

    /// Shows this view of the model in the frame, without asking anything.
    pub fn show_list(&self, id: &str) {
        self.show_view_now(id);
    }

    /// What asks before actions run, for a screen with no window of the framework around it.
    pub fn confirmations(&self) -> ConfirmDialog {
        self.0.confirm.clone()
    }

    /// Registers a control by name, for a dashboard to show.
    pub fn register_control(
        &self,
        name: impl Into<String>,
        control: impl Fn() -> ravenrs_widgets::AnyView + 'static,
    ) {
        self.0.controls.borrow_mut().insert(name.into(), Rc::new(control));
    }

    pub(crate) fn control(&self, name: &str) -> Option<ControlFactory> {
        self.0.controls.borrow().get(name).cloned()
    }

    /// Shows this in a window over the screens, until it is closed.
    pub fn present(
        &self,
        title: impl Into<String>,
        content: impl Fn() -> ravenrs_widgets::AnyView + 'static,
    ) {
        self.0.presented.set(Some((title.into(), Rc::new(content))));
    }

    pub(crate) fn presented(&self) -> &State<Option<Presented>> {
        &self.0.presented
    }

    /// Says what a field of a form is called when that depends on the record.
    pub fn set_form_caption(&self, caption: FormCaption) {
        *self.0.form_caption.borrow_mut() = Some(caption);
    }

    pub(crate) fn form_caption(&self) -> Option<FormCaption> {
        self.0.form_caption.borrow().clone()
    }

    /// Keeps what is wrong with a field on the field alone, for a form short enough to see it.
    pub(crate) fn set_lists_field_errors(&self, lists: bool) {
        self.0.lists_field_errors.set(lists);
    }

    pub(crate) fn lists_field_errors(&self) -> bool {
        self.0.lists_field_errors.get()
    }

    /// The record on the form as it stands, following its fields.
    pub fn form_record(&self) -> Option<Object> {
        self.form().map(|form| form.watched())
    }

    /// The ways to see this list, as their id and what people see.
    pub(crate) fn variants(&self, view: &str) -> Vec<(String, String)> {
        let model = self.model();
        model.at(&["Views", view, "Variants"]).map_or_else(Vec::new, |variants| {
            variants
                .in_order()
                .iter()
                .map(|item| (item.id.clone(), item.text("Caption").unwrap_or(&item.id).to_owned()))
                .collect()
        })
    }

    /// What the way picked to see this list asks of its records. Without one picked, the first.
    pub(crate) fn variant_filter(&self, view: &str) -> Option<String> {
        let model = self.model();
        let variants = model.at(&["Views", view, "Variants"])?;
        let current = model.at(&["Views", view]).and_then(|node| node.text("CurrentVariant"));
        let order = variants.in_order();
        let picked = current.and_then(|id| order.iter().find(|item| item.id == id)).or(order.first())?;
        picked.text("Filter").map(str::to_owned)
    }

    /// Picks a way to see this list, which stays in the person's own layer of the model.
    pub(crate) fn choose_variant(&self, view: &str, variant: &str) -> Result<(), String> {
        self.0
            .application
            .borrow_mut()
            .set_user_setting(&["Views", view], "CurrentVariant", variant)
            .map_err(|e| e.0)?;
        self.touch();
        Ok(())
    }

    /// What the person changed in the model, to keep between runs.
    pub fn user_settings(&self) -> Option<String> {
        let mut application = self.0.application.borrow_mut();
        application.layers_mut().difference(&ravenrs_app::ModelLayer::User).map(|diff| diff.to_json())
    }

    /// Puts this record on the form, as it is, without looking in storage.
    pub(crate) fn open_object(&self, entity: &EntityInfo, object: Object, is_new: bool) {
        match self.detail_of(entity) {
            Ok(view) => self.show(view),
            Err(error) => return self.0.error.set(Some(error)),
        }
        self.0.form.set(Some(Rc::new(FormState::new(entity.clone(), object, is_new))));
    }

    /// Changes a field of the form.
    pub(crate) fn set_form_value(&self, field: &str, value: Value) {
        if let Some(state) = self.0.form.peek().as_ref().and_then(|form| form.fields.get(field).cloned()) {
            state.set(value);
        }
    }

    /// The record on the form, if it meets its rules. If not, the form says what is wrong.
    pub(crate) fn check_form(&self) -> Option<(Object, bool)> {
        let form = self.0.form.peek().clone()?;
        let object = form.current();
        if self.violations_of(&object, CONTEXT_SAVE).is_empty() {
            self.0.checking.set(false);
            Some((object, form.is_new))
        } else {
            self.0.checking.set(true);
            None
        }
    }

    /// The session an action is running in.
    pub fn of(cx: &ActionContext) -> Option<Self> {
        Self::of_frame(&cx.frame)
    }

    /// The session of a frame.
    pub fn of_frame(frame: &Frame) -> Option<Self> {
        frame.extension::<SessionLink>().and_then(|link| link.0.upgrade()).map(Self)
    }

    /// The main frame.
    pub fn frame(&self) -> Frame {
        self.0.frame.clone()
    }

    /// The words of the screens.
    pub fn words(&self) -> &Words {
        &self.0.words
    }

    pub(crate) fn editors(&self) -> &EditorRegistry {
        &self.0.editors
    }

    /// The day pickers open on.
    pub(crate) fn today(&self) -> Date {
        self.0.today.get()
    }

    /// The model as it stands.
    pub fn model(&self) -> Rc<ModelNode> {
        self.0.application.borrow().model()
    }

    /// A session with no window around it, for a piece of a screen.
    pub(crate) fn standalone(application: Application, new_space: SpaceFactory, words: Words) -> Self {
        Self::with_editors(application, new_space, words, EditorRegistry::default())
    }

    /// The same, with controls of its own.
    pub(crate) fn with_editors(
        application: Application,
        new_space: SpaceFactory,
        words: Words,
        editors: EditorRegistry,
    ) -> Self {
        let today = Date::from_days(ravenrs_criteria::DateTime::now().days);
        Self::new(application, new_space, words, editors, today)
    }

    /// Tells lists to read again, after something changed the records from outside.
    pub fn records_changed(&self) {
        self.touch();
    }

    /// A frame of its own for a window over this one, with a unit of work of its own.
    pub(crate) fn new_frame(&self, context: FrameContext) -> Frame {
        self.0.application.borrow().new_frame(context, (self.0.new_space)())
    }

    /// The title of the application.
    pub fn title(&self) -> String {
        self.0.application.borrow().title().to_owned()
    }

    /// An entity by name.
    pub fn entity(&self, name: &str) -> Option<EntityInfo> {
        self.0.application.borrow().entities().iter().find(|e| e.name == name).cloned()
    }

    pub(crate) fn revision(&self) -> u64 {
        self.0.revision.get()
    }

    pub(crate) fn touch(&self) {
        self.0.revision.update(|r| *r += 1);
    }

    pub(crate) fn form(&self) -> Option<Rc<FormState>> {
        self.0.form.get()
    }

    /// What this record fails in this context.
    pub fn violations_of(&self, record: &Object, context: &str) -> Vec<Violation> {
        let Some(entity) = self.entity(record.entity()) else { return Vec::new() };
        let space = self.0.frame.space();
        let space = space.borrow();
        validate(&entity, record, context, &*space, &self.words().rules)
    }

    /// What the form fails, once a save has found something. Follows the fields.
    pub(crate) fn form_violations(&self) -> Vec<Violation> {
        if !self.0.checking.get() {
            return Vec::new();
        }
        self.form().map_or_else(Vec::new, |form| self.violations_of(&form.watched(), CONTEXT_SAVE))
    }

    /// What went wrong last, in words for people.
    pub fn error(&self) -> Option<String> {
        self.0.error.get()
    }

    pub(crate) fn set_error(&self, error: Option<String>) {
        self.0.error.set(error);
    }

    pub(crate) fn confirm(&self) -> &ConfirmDialog {
        &self.0.confirm
    }

    /// Hands exported files over to this, as a name and its bytes.
    pub(crate) fn set_on_file(&self, handler: FileHandler) {
        *self.0.on_file.borrow_mut() = Some(handler);
    }

    /// Hands addresses to open over to this.
    pub(crate) fn set_on_url(&self, handler: UrlHandler) {
        *self.0.on_url.borrow_mut() = Some(handler);
    }

    /// Hands a file over to whoever asked for them, or else to the person.
    pub(crate) fn hand_file(&self, name: String, bytes: Vec<u8>, picker: &FilePicker) {
        let handler = self.0.on_file.borrow().clone();
        match handler {
            Some(handler) => handler(name, bytes),
            None => picker.save(&name, &bytes),
        }
    }

    /// Whether a save found something on the form.
    pub(crate) fn is_checking(&self) -> bool {
        *self.0.checking.peek()
    }

    /// Whether there is a form with changes that were not kept.
    pub fn has_unsaved_changes(&self) -> bool {
        self.0.form.peek().as_ref().is_some_and(|form| form.is_dirty())
    }

    /// Runs `then`, asking first if it would lose changes.
    pub(crate) fn leaving(&self, then: impl Fn() + 'static) {
        if self.has_unsaved_changes() {
            let words = self.words();
            self.0.confirm.ask(
                words.discard_question.clone(),
                words.discard_detail.clone(),
                words.discard.clone(),
                then,
            );
        } else {
            then();
        }
    }

    fn show(&self, view: ViewInfo) {
        self.0.error.set(None);
        self.0.checking.set(false);
        self.0.frame.set_view_in(Some(view), (self.0.new_space)());
    }

    /// Shows a list, asking first if that would lose changes.
    pub fn open_view(&self, id: &str) {
        let (session, id) = (self.clone(), id.to_owned());
        self.leaving(move || session.show_view_now(&id));
    }

    pub(crate) fn show_view_now(&self, id: &str) {
        let model = self.model();
        match ViewInfo::from_model(&model, id) {
            Ok(view) => {
                self.0.form.set(None);
                self.0.history.set(vec![id.to_owned()]);
                self.show(view);
                self.touch();
            }
            Err(error) => self.0.error.set(Some(error.0)),
        }
    }

    fn detail_of(&self, entity: &EntityInfo) -> Result<ViewInfo, String> {
        let id = format!("{}_DetailView", entity.name);
        ViewInfo::from_model(&self.model(), &id).map_err(|e| e.0)
    }

    fn remember_siblings(&self, entity: &EntityInfo) {
        let Some(field) = entity.key_field() else { return };
        let space = self.0.frame.space();
        let found = space.borrow().query(&entity.name, &Query::all());
        *self.0.siblings.borrow_mut() =
            found.map_or_else(|_| Vec::new(), |all| all.iter().map(|o| o.get(&field.name)).collect());
    }

    /// Opens the form of a record, in a unit of work of its own.
    pub fn open_detail(&self, entity_name: &str, key: &Value) {
        let Some(entity) = self.entity(entity_name) else { return };
        let view = match self.detail_of(&entity) {
            Ok(view) => view,
            Err(error) => return self.0.error.set(Some(error)),
        };
        if self.0.frame.view().is_some_and(|v| v.kind == ViewKind::List) {
            self.remember_siblings(&entity);
            self.0.history.update(|history| history.push(view.id.clone()));
        }
        self.show(view);
        let found = self.0.frame.space().borrow().find(entity_name, key);
        match found {
            Ok(Some(object)) => {
                self.0.frame.set_selected(vec![object.clone()]);
                self.0.form.set(Some(Rc::new(FormState::new(entity.clone(), object, false))));
            }
            Ok(None) => self.0.error.set(Some(format!("{} {}", entity.caption, key_text(&entity, key)))),
            Err(error) => self.0.error.set(Some(error.to_string())),
        }
        self.update_position(&entity);
    }

    /// Opens the form of a new record.
    pub fn open_new(&self, entity_name: &str) {
        let Some(entity) = self.entity(entity_name) else { return };
        let view = match self.detail_of(&entity) {
            Ok(view) => view,
            Err(error) => return self.0.error.set(Some(error)),
        };
        if self.0.frame.view().is_some_and(|v| v.kind == ViewKind::List) {
            self.0.history.update(|history| history.push(view.id.clone()));
        }
        self.show(view);
        let made = self.0.frame.space().borrow_mut().new_object(entity_name);
        match made {
            Ok(object) => self.0.form.set(Some(Rc::new(FormState::new(entity.clone(), object, true)))),
            Err(error) => self.0.error.set(Some(error.to_string())),
        }
        self.update_position(&entity);
    }

    /// Turns moving between records on and off, by where this one is among the others.
    fn update_position(&self, entity: &EntityInfo) {
        let at = self.position(entity);
        let last = self.0.siblings.borrow().len().saturating_sub(1);
        if let Some(action) = self.0.frame.action(crate::ids::PREVIOUS) {
            action.set_enabled("position", at.is_some_and(|at| at > 0));
        }
        if let Some(action) = self.0.frame.action(crate::ids::NEXT) {
            action.set_enabled("position", at.is_some_and(|at| at < last));
        }
    }

    fn position(&self, entity: &EntityInfo) -> Option<usize> {
        let form = self.0.form.peek().clone()?;
        if form.is_new {
            return None;
        }
        let key = entity.key_field().map(|f| form.original.get(&f.name))?;
        self.0.siblings.borrow().iter().position(|k| *k == key)
    }

    /// Moves to the record this far from the current one, asking first if that would lose changes.
    pub(crate) fn step(&self, by: isize) {
        let Some(form) = self.0.form.peek().clone() else { return };
        let Some(at) = self.position(&form.entity) else { return };
        let Some(to) = at.checked_add_signed(by) else { return };
        let Some(key) = self.0.siblings.borrow().get(to).cloned() else { return };
        let (session, name) = (self.clone(), form.entity.name.clone());
        self.leaving(move || session.open_detail(&name, &key));
    }

    /// Keeps what the form holds. Stays where it is; what is wrong goes on the screen.
    pub(crate) fn save(&self) -> Result<(), String> {
        let Some(form) = self.0.form.peek().clone() else { return Ok(()) };
        let object = form.current();
        let found = self.violations_of(&object, CONTEXT_SAVE);
        if !found.is_empty() {
            self.0.error.set(None);
            self.0.checking.set(true);
            return Err(found.into_iter().map(|v| v.message).collect::<Vec<_>>().join("; "));
        }
        self.0.checking.set(false);
        let space = self.0.frame.space();
        let done = {
            let mut space = space.borrow_mut();
            let kept = if form.is_new { space.insert(object.clone()) } else { space.save(object.clone()) };
            kept.and_then(|()| space.commit())
        };
        match done {
            Ok(()) => {
                self.0.error.set(None);
                let saved = form.entity.key_field().map(|f| object.get(&f.name));
                self.touch();
                if let Some(key) = saved {
                    // The same record, no longer new or changed.
                    self.remember_siblings(&form.entity);
                    let found = space.borrow().find(&form.entity.name, &key).ok().flatten();
                    let fresh = found.unwrap_or(object);
                    self.0.frame.set_selected(vec![fresh.clone()]);
                    self.0.form.set(Some(Rc::new(FormState::new(form.entity.clone(), fresh, false))));
                    self.update_position(&form.entity);
                }
                Ok(())
            }
            Err(error) => {
                // What it could not keep must not stay waiting for the next try.
                space.borrow_mut().rollback();
                let message = error.to_string();
                self.0.error.set(Some(message.clone()));
                Err(message)
            }
        }
    }

    /// Goes back to where the form came from, without asking.
    pub(crate) fn back_now(&self) {
        self.0.form.set(None);
        let previous = {
            let mut history = self.0.history.get();
            history.pop();
            let previous = history.last().cloned();
            self.0.history.set(history);
            previous
        };
        let target = previous.or_else(|| self.first_view());
        if let Some(id) = target {
            self.show_view_now(&id);
        }
    }

    /// Goes back, asking first if that would lose changes.
    pub(crate) fn back(&self) {
        let session = self.clone();
        self.leaving(move || session.back_now());
    }

    /// Throws away what the form changed, and leaves, which is what was asked.
    pub(crate) fn cancel(&self) {
        self.0.frame.space().borrow_mut().rollback();
        self.back_now();
    }

    /// Takes away these records and keeps that, in the frame's unit of work.
    pub(crate) fn delete(&self, objects: &[Object]) -> Result<(), String> {
        let refused: Vec<String> = objects
            .iter()
            .flat_map(|object| self.violations_of(object, CONTEXT_DELETE))
            .map(|v| v.message)
            .collect();
        if !refused.is_empty() {
            return Err(refused.join("; "));
        }
        let space = self.0.frame.space();
        let done = {
            let mut space = space.borrow_mut();
            let mut result = Ok(());
            for object in objects {
                let Some(entity) = self.entity(object.entity()) else { continue };
                let Some(field) = entity.key_field() else { continue };
                result = result.and_then(|()| space.delete(&entity.name, &object.get(&field.name)));
            }
            result.and_then(|()| space.commit())
        };
        match done {
            Ok(()) => {
                self.touch();
                let leaving_form = self.0.form.peek().is_some();
                if leaving_form {
                    self.back_now();
                } else {
                    self.0.frame.set_selected(Vec::new());
                }
                Ok(())
            }
            Err(error) => {
                space.borrow_mut().rollback();
                let message = error.to_string();
                self.0.error.set(Some(message.clone()));
                Err(message)
            }
        }
    }

    /// Reads again. A form goes back to what is kept, asking first if that would lose changes.
    pub(crate) fn refresh(&self) {
        let session = self.clone();
        self.leaving(move || {
            session.0.frame.space().borrow_mut().refresh();
            session.touch();
            let form = session.0.form.peek().clone();
            if let Some(form) = form {
                if form.is_new {
                    session.back_now();
                } else if let Some(field) = form.entity.key_field() {
                    let key = form.original.get(&field.name);
                    session.open_detail(&form.entity.name, &key);
                }
            }
        });
    }

    /// The first thing in the navigation, where the application opens.
    pub(crate) fn first_view(&self) -> Option<String> {
        fn first(node: &ModelNode) -> Option<String> {
            node.in_order()
                .into_iter()
                .find_map(|item| item.text("View").map(str::to_owned).or_else(|| first(item)))
        }
        first(self.model().get("NavigationItems")?)
    }

    /// Runs an action from a screen: asks its question first, and does what it asks for next.
    pub fn run_action(&self, action: &Action, input: ActionInput) {
        let Some(question) = action.confirmation_text().map(str::to_owned) else {
            self.execute(action.id(), input);
            return;
        };
        let (session, id) = (self.clone(), action.id().to_owned());
        let detail = action.confirmation_detail().unwrap_or(&self.words().delete_detail).to_owned();
        let answer = action.confirmation_answer().unwrap_or(action.caption()).to_owned();
        self.0.confirm.ask(question, detail, answer, move || {
            session.execute(&id, input.clone());
        });
    }

    fn execute(&self, id: &str, input: ActionInput) {
        match self.0.frame.execute(id, input) {
            Ok(ActionResult::OpenUrl(address)) => {
                let handler = self.0.on_url.borrow().clone();
                if let Some(handler) = handler {
                    handler(address);
                }
            }
            Ok(_) => {}
            // The form already says what is wrong, on each field.
            Err(_) if *self.0.checking.peek() => {}
            Err(error) => self.0.error.set(Some(error.to_string())),
        }
    }
}
