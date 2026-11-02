//! What the screens of a running application share.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

use ravenrs_app::{
    Action, ActionContext, ActionInput, ActionResult, Application, EntityInfo, Frame, FrameContext,
    ModelNode, Object, ObjectSpace, Query, Value, ViewInfo, ViewKind,
};
use ravenrs_reactive::State;
use ravenrs_widgets::{ConfirmDialog, Date};

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

struct SessionInner {
    application: RefCell<Application>,
    new_space: SpaceFactory,
    frame: Frame,
    words: Words,
    editors: EditorRegistry,
    /// Goes up whenever records may have changed, so lists read again.
    revision: State<u64>,
    form: State<Option<Rc<FormState>>>,
    /// Where it came from, for going back.
    history: State<Vec<String>>,
    error: State<Option<String>>,
    confirm: ConfirmDialog,
    on_file: RefCell<Option<FileHandler>>,
    on_url: RefCell<Option<UrlHandler>>,
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
        let inner = Rc::new(SessionInner {
            application: RefCell::new(application),
            new_space,
            frame: frame.clone(),
            words,
            editors,
            revision: State::new(0),
            form: State::new(None),
            history: State::new(Vec::new()),
            error: State::new(None),
            confirm: ConfirmDialog::new(),
            on_file: RefCell::default(),
            on_url: RefCell::default(),
            today: Cell::new(today),
            siblings: RefCell::default(),
        });
        frame.set_extension(Rc::new(SessionLink(Rc::downgrade(&inner))));
        Self(inner)
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

    fn touch(&self) {
        self.0.revision.update(|r| *r += 1);
    }

    pub(crate) fn form(&self) -> Option<Rc<FormState>> {
        self.0.form.get()
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

    pub(crate) fn hand_file(&self, name: String, bytes: Vec<u8>) {
        let handler = self.0.on_file.borrow().clone();
        if let Some(handler) = handler {
            handler(name, bytes);
        }
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
        let (session, id, words) = (self.clone(), action.id().to_owned(), self.words().clone());
        self.0.confirm.ask(question, words.delete_detail, action.caption().to_owned(), move || {
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
            Err(error) => self.0.error.set(Some(error.to_string())),
        }
    }
}
