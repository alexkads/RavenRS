//! What the screens of a running application share.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use ravenrs_app::{
    Action, ActionContext, ActionInput, ActionResult, Application, AttachmentStore, CONTEXT_DELETE,
    CONTEXT_SAVE, Criteria, EntityInfo, FieldKind, Frame, FrameContext, Message, MessageKind, ModelLayer,
    ModelNode, ModelStore, Object, ObjectSpace, Query, RuleWords, Severity, Value, ViewInfo, ViewKind,
    Violation, merge_changes, validate,
};
use ravenrs_reactive::State;
use ravenrs_widgets::{ConfirmDialog, Date, FilePicker, ToastExtras, Toaster, Toasts, Tone};

use crate::editors::EditorRegistry;
use crate::values::key_text;

/// The words the generated screens use.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Words {
    /// Lists the keys of what can be done.
    pub shortcuts: String,
    /// Gets back to a locked window.
    pub unlock: String,
    /// What was typed on a locked window is not it.
    pub unlock_wrong: String,
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
    /// The menu of the filters ready on a list.
    pub filter: String,
    /// Makes a new record from the one selected.
    pub clone: String,
    /// Why a record cannot be cloned while it has changes.
    pub clone_unsaved: String,
    /// Takes a file out, to wherever people say.
    pub save_file: String,
    /// The question when rules only warn.
    pub warning_question: String,
    /// Goes on in spite of what rules warn about.
    pub warning_answer: String,
    /// The question when someone else changed the same fields, where `{0}` is which ones.
    pub conflict_question: String,
    /// What keeping one's own changes does, where `{0}` is which fields.
    pub conflict_detail: String,
    /// Keeps one's own changes over the other person's.
    pub conflict_answer: String,
    /// What a way of showing a list says when something it needs is missing.
    pub list_editor_needs: String,
    /// A row of a timeline for records that point to no resource.
    pub no_resource: String,
    /// The series of a chart that counts records.
    pub chart_count: String,
    /// Things to be reminded of.
    pub reminders: String,
    /// Reminds again later.
    pub snooze: String,
    /// Stops reminding.
    pub dismiss: String,
    /// Opens what a reminder is about.
    pub open: String,
    /// A field that is on.
    pub yes: String,
    /// A field that is off.
    pub no: String,
    /// What people are told when something is refused.
    pub access_denied: String,
    /// What shows in place of a field the person cannot read.
    pub protected_value: String,
    /// Enters the application.
    pub sign_in: String,
    /// Leaves the application.
    pub sign_out: String,
    /// Says signing in did not work, without saying why.
    pub sign_in_failed: String,
    /// The name someone signs in with.
    pub user_name: String,
    /// Their password.
    pub password: String,
    /// The password they have now.
    pub current_password: String,
    /// The password they want.
    pub new_password: String,
    /// The same again.
    pub repeat_password: String,
    /// Signing in could not be tried.
    pub sign_in_unavailable: String,
    /// The two are not the same.
    pub passwords_differ: String,
    /// It is too short.
    pub password_too_short: String,
    /// The current one is not right.
    pub password_wrong_current: String,
    /// It was changed.
    pub password_saved: String,
    /// Keeps the new password.
    pub save_password: String,
    /// Changes one's own password.
    pub change_password: String,
    /// Gives someone a password.
    pub set_password: String,
    /// Why they have to choose one now.
    pub must_change_password: String,
    /// What the first screen of a new installation says.
    pub first_user: String,
    /// Creates the first person.
    pub create_user: String,
    /// The editor of the application's model.
    pub model_editor: String,
    /// Looks for a node of the model.
    pub search_model: String,
    /// What shows before a node is picked.
    pub pick_model_node: String,
    /// Applies what was typed.
    pub apply: String,
    /// Throws away the person's own changes to the model.
    pub restore_model: String,
    /// Why what is on a form cannot be kept any more.
    pub record_gone: String,
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
            shortcuts: "Keyboard shortcuts".into(),
            unlock: "Unlock".into(),
            unlock_wrong: "That is not it. Try again.".into(),
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
            filter: "Filter".into(),
            clone: "Clone".into(),
            clone_unsaved: "Save the changes before cloning.".into(),
            save_file: "Save file".into(),
            warning_question: "Save anyway?".into(),
            warning_answer: "Save anyway".into(),
            conflict_question: "Someone else changed this record".into(),
            conflict_detail: "They changed {0} too. Saving keeps your value there, and their other changes."
                .into(),
            conflict_answer: "Keep mine".into(),
            list_editor_needs: "This view needs a field that fits:".into(),
            no_resource: "No resource".into(),
            chart_count: "Records".into(),
            reminders: "Reminders".into(),
            snooze: "In 15 minutes".into(),
            dismiss: "Dismiss".into(),
            open: "Open".into(),
            yes: "Yes".into(),
            no: "No".into(),
            access_denied: "You are not allowed to do this.".into(),
            protected_value: "Protected".into(),
            sign_in: "Sign in".into(),
            sign_out: "Sign out".into(),
            sign_in_failed:
                "The name or the password is not right, or the account is waiting after too many attempts."
                    .into(),
            user_name: "Name".into(),
            password: "Password".into(),
            current_password: "Current password".into(),
            new_password: "New password".into(),
            repeat_password: "Repeat the password".into(),
            sign_in_unavailable: "Signing in is not available right now. Try again.".into(),
            passwords_differ: "The two passwords are not the same.".into(),
            password_too_short: "A password has at least 8 characters.".into(),
            password_wrong_current: "The current password is not right.".into(),
            password_saved: "Password changed.".into(),
            save_password: "Save password".into(),
            change_password: "Change password".into(),
            set_password: "Set password".into(),
            must_change_password: "This password was given to you. Choose your own to go on.".into(),
            first_user: "Nobody can sign in yet. This creates the administrator.".into(),
            create_user: "Create administrator".into(),
            model_editor: "Application model".into(),
            search_model: "Search the model".into(),
            pick_model_node: "Pick a node to see its settings.".into(),
            apply: "Apply".into(),
            restore_model: "Restore all".into(),
            record_gone: "Someone else deleted this record.".into(),
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
            shortcuts: "Consulta de atalhos".into(),
            unlock: "Desbloquear".into(),
            unlock_wrong: "Não confere. Tente de novo.".into(),
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
            filter: "Filtro".into(),
            clone: "Clonar".into(),
            clone_unsaved: "Salve as alterações antes de clonar.".into(),
            save_file: "Salvar arquivo".into(),
            warning_question: "Salvar assim mesmo?".into(),
            warning_answer: "Salvar assim mesmo".into(),
            conflict_question: "Outra pessoa alterou este registro".into(),
            conflict_detail:
                "Ela também alterou {0}. Salvar mantém o seu valor ali, e as outras alterações dela.".into(),
            conflict_answer: "Manter o meu".into(),
            list_editor_needs: "Esta visão precisa de um campo que sirva:".into(),
            no_resource: "Sem recurso".into(),
            chart_count: "Registros".into(),
            reminders: "Lembretes".into(),
            snooze: "Em 15 minutos".into(),
            dismiss: "Dispensar".into(),
            open: "Abrir".into(),
            yes: "Sim".into(),
            no: "Não".into(),
            access_denied: "Você não tem permissão para fazer isso.".into(),
            protected_value: "Protegido".into(),
            sign_in: "Entrar".into(),
            sign_out: "Sair".into(),
            sign_in_failed:
                "O nome ou a senha não conferem, ou a conta está aguardando depois de muitas tentativas."
                    .into(),
            user_name: "Nome".into(),
            password: "Senha".into(),
            current_password: "Senha atual".into(),
            new_password: "Nova senha".into(),
            repeat_password: "Repita a senha".into(),
            sign_in_unavailable: "Não foi possível entrar agora. Tente de novo.".into(),
            passwords_differ: "As duas senhas não são iguais.".into(),
            password_too_short: "A senha precisa ter pelo menos 8 caracteres.".into(),
            password_wrong_current: "A senha atual não confere.".into(),
            password_saved: "Senha alterada.".into(),
            save_password: "Salvar senha".into(),
            change_password: "Trocar senha".into(),
            set_password: "Definir senha".into(),
            must_change_password: "Esta senha foi definida por outra pessoa. Escolha a sua para continuar."
                .into(),
            first_user: "Ninguém consegue entrar ainda. Isto cria o administrador.".into(),
            create_user: "Criar administrador".into(),
            model_editor: "Modelo da aplicação".into(),
            search_model: "Buscar no modelo".into(),
            pick_model_node: "Escolha um nó para ver as configurações.".into(),
            apply: "Aplicar".into(),
            restore_model: "Restaurar tudo".into(),
            record_gone: "Outra pessoa excluiu este registro.".into(),
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
        self.fields.iter().any(|(name, value)| *value.peek() != self.original.get(name))
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

/// Gives the records of a collection with no storage, from the record that has it.
pub type CollectionSource = Rc<dyn Fn(&Object) -> Vec<Object>>;

/// Makes a control an application registered.
pub(crate) type ControlFactory = Rc<dyn Fn() -> ravenrs_widgets::AnyView>;

/// Something shown in a window over the screens, with its title.
pub(crate) type Presented = (String, ControlFactory);

/// What it takes to get back to a locked window.
#[derive(Clone)]
pub(crate) struct Lock {
    pub(crate) title: String,
    pub(crate) check: Rc<dyn Fn(&str) -> bool>,
}

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
    /// A save went through with notes from its rules, which the form keeps showing.
    noting: State<bool>,
    toasts: Toasts,
    /// Where entities no storage keeps get their records.
    record_sources: Rc<RefCell<BTreeMap<String, crate::list_view::ListSource>>>,
    /// Where lists no storage keeps get theirs, by entity and field.
    collection_sources: Rc<RefCell<BTreeMap<(String, String), CollectionSource>>>,
    /// Where it came from, for going back.
    history: State<Vec<String>>,
    error: State<Option<String>>,
    confirm: ConfirmDialog,
    on_file: Rc<RefCell<Option<FileHandler>>>,
    on_url: Rc<RefCell<Option<UrlHandler>>>,
    controls: Rc<RefCell<BTreeMap<String, ControlFactory>>>,
    presented: State<Option<Presented>>,
    jobs: crate::background::SharedJobs,
    locked: State<Option<Lock>>,
    /// Something that takes the place of the view, as an editor of its own does.
    full: State<Option<ControlFactory>>,
    /// How many reminders are due, and whether they are showing.
    reminders: State<(usize, bool)>,
    form_caption: RefCell<Option<FormCaption>>,
    /// Whether the form repeats, above it, what it says on each field.
    lists_field_errors: Cell<bool>,
    list_editors: Rc<RefCell<BTreeMap<String, crate::list_view::ListEditorFactory>>>,
    attachment_store: Rc<RefCell<Option<Rc<dyn AttachmentStore>>>>,
    model_store: Rc<RefCell<Option<Rc<dyn ModelStore>>>>,
    /// What the list of the popup about to open has to meet.
    popup_filter: RefCell<Option<Criteria>>,
    /// Records that come with the one on the form, kept when it is.
    pending: RefCell<Vec<Object>>,
    /// The fields the keyboard has left, whose errors a form that checks on leaving shows.
    left: State<BTreeSet<String>>,
    today: Cell<Date>,
    /// The keys of the list a form came from, for moving between records.
    siblings: RefCell<Vec<Value>>,
    /// Who is signed in and how they leave.
    account: RefCell<Option<Account>>,
}

/// Who is signed in, for the window to show.
#[derive(Clone)]
pub(crate) struct Account {
    pub(crate) name: String,
    pub(crate) sign_out: Rc<dyn Fn()>,
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
            noting: State::new(false),
            toasts: Toasts::new(),
            record_sources: Rc::default(),
            collection_sources: Rc::default(),
            history: State::new(Vec::new()),
            error: State::new(None),
            confirm: ConfirmDialog::new(),
            on_file: Rc::default(),
            on_url: Rc::default(),
            controls: Rc::default(),
            presented: State::new(None),
            jobs: Rc::new(crate::background::Jobs::default()),
            locked: State::new(None),
            full: State::new(None),
            reminders: State::new((0, false)),
            form_caption: RefCell::default(),
            lists_field_errors: Cell::new(true),
            list_editors: Rc::default(),
            attachment_store: Rc::default(),
            model_store: Rc::default(),
            popup_filter: RefCell::default(),
            pending: RefCell::default(),
            left: State::new(BTreeSet::new()),
            today: Cell::new(today),
            siblings: RefCell::default(),
            account: RefCell::default(),
        })
    }

    fn linked(inner: SessionInner) -> Self {
        let is_first = inner.list_editors.borrow().is_empty();
        let inner = Rc::new(inner);
        let frame = inner.frame.clone();
        frame.set_extension(Rc::new(SessionLink(Rc::downgrade(&inner))));
        let telling = Rc::downgrade(&inner);
        frame.set_messenger(move |message| {
            if let Some(inner) = telling.upgrade() {
                Session(inner).show_message(message);
            }
        });
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
                    let found = session.violations_of(record, context);
                    messages.extend(found.into_iter().filter(Violation::is_error).map(|v| v.message));
                }
            }
            if messages.is_empty() { Ok(()) } else { Err(messages.join("; ")) }
        });
        let session = Self(inner);
        if is_first {
            crate::list_editors::register(&session);
        }
        session
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
            noting: State::new(false),
            toasts: parent.toasts.clone(),
            record_sources: parent.record_sources.clone(),
            collection_sources: parent.collection_sources.clone(),
            history: State::new(Vec::new()),
            error: State::new(None),
            confirm: ConfirmDialog::new(),
            on_file: parent.on_file.clone(),
            on_url: parent.on_url.clone(),
            controls: parent.controls.clone(),
            presented: parent.presented.clone(),
            jobs: parent.jobs.clone(),
            locked: parent.locked.clone(),
            full: parent.full.clone(),
            reminders: parent.reminders.clone(),
            form_caption: RefCell::new(parent.form_caption.borrow().clone()),
            lists_field_errors: Cell::new(parent.lists_field_errors.get()),
            list_editors: parent.list_editors.clone(),
            attachment_store: parent.attachment_store.clone(),
            model_store: parent.model_store.clone(),
            popup_filter: RefCell::default(),
            pending: RefCell::default(),
            left: State::new(BTreeSet::new()),
            today: Cell::new(parent.today.get()),
            siblings: RefCell::default(),
            account: RefCell::default(),
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

    /// What shows messages, for a screen with no window of the framework around it.
    pub fn toaster(&self) -> Toaster {
        Toaster::new(self.0.toasts.clone())
    }

    /// Tells people something, which goes away by itself.
    pub fn show_message(&self, message: Message) {
        let tone = match message.kind {
            MessageKind::Success => Tone::Success,
            MessageKind::Warning => Tone::Warning,
            MessageKind::Error => Tone::Danger,
            _ => Tone::Accent,
        };
        let mut extras = ToastExtras::new();
        if let Some((label, run)) = message.action {
            extras = extras.action(label, move || run());
        }
        if let Some(run) = message.on_close {
            extras = extras.on_close(move || run());
        }
        self.0.toasts.show_with(self.title(), message.text, tone, message.seconds.unwrap_or(4.0), extras);
    }

    /// Gives the records of an entity no storage keeps, for its list and its forms.
    pub fn use_records(&self, entity: impl Into<String>, source: impl Fn() -> Vec<Object> + 'static) {
        self.0.record_sources.borrow_mut().insert(entity.into(), Rc::new(source));
    }

    /// Gives the records of a list no storage keeps, inside a form of this entity.
    pub fn use_collection(
        &self,
        entity: impl Into<String>,
        field: impl Into<String>,
        source: impl Fn(&Object) -> Vec<Object> + 'static,
    ) {
        self.0.collection_sources.borrow_mut().insert((entity.into(), field.into()), Rc::new(source));
    }

    pub(crate) fn record_source(&self, entity: &str) -> Option<crate::list_view::ListSource> {
        self.0.record_sources.borrow().get(entity).cloned()
    }

    pub(crate) fn collection_source(&self, entity: &str, field: &str) -> Option<CollectionSource> {
        self.0.collection_sources.borrow().get(&(entity.to_owned(), field.to_owned())).cloned()
    }

    /// What people see in the model, as CSV with an empty column for its translation.
    pub fn export_captions(&self) -> String {
        self.0.application.borrow().export_captions()
    }

    /// Takes the CSV of [`export_captions`](Self::export_captions), filled in, as the model in this
    /// language, which [`set_language`](Self::set_language) picks.
    pub fn import_captions(&self, language: &str, csv: &str) -> Result<(), String> {
        self.0.application.borrow_mut().import_captions(language, csv).map_err(|e| e.0)
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

    pub(crate) fn account(&self) -> Option<Account> {
        self.0.account.borrow().clone()
    }

    pub(crate) fn use_account(&self, account: Account) {
        *self.0.account.borrow_mut() = Some(account);
    }

    /// Whether this navigation item leads somewhere this person can go. It only hides: what
    /// protects records is the secured space.
    pub(crate) fn can_open(&self, group: Option<&str>, item: &str, view: &str) -> bool {
        let Some(security) = self.0.frame.security() else { return true };
        let model = self.model();
        let entity = model.at(&["Views", view]).and_then(|node| node.text("Entity"));
        security.with(|permissions| {
            permissions.can_navigate(group, item)
                && entity.is_none_or(|entity| permissions.can(ravenrs_app::Operation::Read, entity, None))
                && (view != crate::security::MODEL_EDITOR_VIEW || permissions.can_edit_model())
        })
    }

    /// Changes a setting of the model in the person's own layer, if they may, and keeps it.
    pub fn set_model_setting(&self, path: &[&str], name: &str, value: &str) -> Result<(), String> {
        self.check_model_permission()?;
        self.0.application.borrow_mut().set_typed_setting(path, name, value).map_err(|e| e.0)?;
        self.keep_settings()?;
        self.touch();
        Ok(())
    }

    /// Throws away the person's own changes to the model, if they may.
    pub fn restore_model_settings(&self) -> Result<(), String> {
        self.check_model_permission()?;
        self.0.application.borrow_mut().clear_user_settings().map_err(|e| e.0)?;
        self.keep_settings()?;
        self.touch();
        Ok(())
    }

    fn check_model_permission(&self) -> Result<(), String> {
        match self.0.frame.security() {
            Some(security) if !security.with(ravenrs_app::Permissions::can_edit_model) => {
                Err(self.0.words.access_denied.clone())
            }
            _ => Ok(()),
        }
    }

    /// Shows this in place of the view, with the navigation still around it, until
    /// [`close_full`](Self::close_full) or going anywhere else.
    pub fn show_full(&self, content: impl Fn() -> ravenrs_widgets::AnyView + 'static) {
        self.0.full.set(Some(Rc::new(content)));
    }

    /// Goes back to the view from what [`show_full`](Self::show_full) showed.
    pub fn close_full(&self) {
        self.0.full.set(None);
    }

    pub(crate) fn entities_with_reminders(&self) -> Vec<EntityInfo> {
        self.0.application.borrow().entities().iter().filter(|e| e.reminder.is_some()).cloned().collect()
    }

    pub(crate) fn reminders(&self) -> &State<(usize, bool)> {
        &self.0.reminders
    }

    pub(crate) fn full(&self) -> Option<ControlFactory> {
        self.0.full.get()
    }

    pub(crate) fn jobs(&self) -> crate::background::SharedJobs {
        self.0.jobs.clone()
    }

    /// Hides everything until what is typed passes `check`. What people were doing stays as it was.
    pub fn lock(&self, title: impl Into<String>, check: impl Fn(&str) -> bool + 'static) {
        self.0.locked.set(Some(Lock { title: title.into(), check: Rc::new(check) }));
    }

    pub(crate) fn locked(&self) -> &State<Option<Lock>> {
        &self.0.locked
    }

    /// What its modules want everyone to know.
    pub fn notices(&self) -> Vec<ravenrs_app::Notice> {
        self.0.application.borrow().notices()
    }

    /// Shows the keys of what can be done here, in a window over the screens.
    pub fn show_shortcuts(&self) {
        let (title, rows): (String, Vec<(String, String)>) = (
            self.words().shortcuts.clone(),
            self.frame()
                .actions()
                .iter()
                .filter(|action| action.is_active())
                .filter_map(|action| {
                    let keys = action.hotkey_text()?;
                    let label =
                        crate::container::hotkey_of(keys).map_or_else(|| keys.to_owned(), |key| key.label());
                    Some((label, action.caption().to_owned()))
                })
                .collect(),
        );
        self.present(title, move || {
            let lines: Vec<ravenrs_widgets::AnyView> = rows
                .iter()
                .map(|(keys, what)| {
                    Box::new(
                        ravenrs_widgets::HStack::new((
                            ravenrs_widgets::Text::new(keys.clone())
                                .font_weight(ravenrs_style::Weight::SEMIBOLD),
                            ravenrs_widgets::Text::new(what.clone()),
                        ))
                        .spacing(12.0),
                    ) as ravenrs_widgets::AnyView
                })
                .collect();
            Box::new(ravenrs_widgets::VStack::new(lines).spacing(8.0))
        });
    }

    /// Closes what [`present`](Self::present) showed.
    pub fn close_presented(&self) {
        self.0.presented.set(None);
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

    /// What this list offers under `part`, `Variants` or `Filters`, as their id and what people see.
    pub(crate) fn choices(&self, view: &str, part: &str) -> Vec<(String, String)> {
        let model = self.model();
        model.at(&["Views", view, part]).map_or_else(Vec::new, |choices| {
            choices
                .in_order()
                .iter()
                .map(|item| (item.id.clone(), item.text("Caption").unwrap_or(&item.id).to_owned()))
                .collect()
        })
    }

    /// The choice in force under `part`: the one picked or else the first.
    fn chosen(model: &ModelNode, view: &str, part: &str) -> Option<ModelNode> {
        let choices = model.at(&["Views", view, part])?;
        let current = model.at(&["Views", view]).and_then(|node| node.text(&format!("Current{part}")));
        let order = choices.in_order();
        current
            .and_then(|id| order.iter().find(|item| item.id == id))
            .or(order.first())
            .map(|&item| item.clone())
    }

    /// What the model asks of the records of this list: its criteria, and the way to see it and the
    /// filter picked.
    pub(crate) fn model_filter(&self, view: &str) -> Option<Criteria> {
        let model = self.model();
        let own = model.at(&["Views", view]).and_then(|node| node.text("Criteria")).map(str::to_owned);
        let picked = ["Variants", "Filters"].into_iter().map(|part| {
            Self::chosen(&model, view, part).and_then(|item| item.text("Filter").map(str::to_owned))
        });
        std::iter::once(own)
            .chain(picked)
            .flatten()
            .filter_map(|text| Criteria::parse(&text).ok())
            .reduce(Criteria::and)
    }

    /// The dashboard to show for this one: the one people changed it to, if any.
    pub(crate) fn dashboard_shown(&self, view: &str) -> ViewInfo {
        let model = self.model();
        let id = Self::chosen(&model, view, "Variants")
            .and_then(|item| item.text("View").map(str::to_owned))
            .unwrap_or_else(|| view.to_owned());
        ViewInfo::from_model(&model, &id).unwrap_or_else(|_| ViewInfo::new(view, ViewKind::Dashboard, None))
    }

    /// The list editor the way picked to see this list asks for, if it says.
    pub(crate) fn variant_editor(&self, view: &str) -> Option<String> {
        let model = self.model();
        Self::chosen(&model, view, "Variants").and_then(|item| item.text("EditorType").map(str::to_owned))
    }

    /// The fields the way picked to see this list shows, if it says.
    pub(crate) fn variant_columns(&self, view: &str) -> Option<Vec<String>> {
        let item = Self::chosen(&self.model(), view, "Variants")?;
        item.get("Columns").map(|columns| columns.in_order().iter().map(|c| c.id.clone()).collect())
    }

    /// Picks under `part`, which stays in the person's own layer of the model.
    pub(crate) fn choose(&self, view: &str, part: &str, choice: &str) -> Result<(), String> {
        self.0
            .application
            .borrow_mut()
            .set_user_setting(&["Views", view], &format!("Current{part}"), choice)
            .map_err(|e| e.0)?;
        self.keep_settings()?;
        self.touch();
        Ok(())
    }

    /// Keeps the person's own layer of the model there, and reads what is there now.
    pub fn use_model_store(&self, store: Rc<dyn ModelStore>) -> Result<(), String> {
        {
            let mut application = self.0.application.borrow_mut();
            application.layers_mut().load(&ModelLayer::User, &*store).map_err(|e| e.0)?;
            application.refresh_model().map_err(|e| e.0)?;
        }
        *self.0.model_store.borrow_mut() = Some(store);
        self.touch();
        Ok(())
    }

    fn keep_settings(&self) -> Result<(), String> {
        let store = self.0.model_store.borrow().clone();
        let Some(store) = store else { return Ok(()) };
        let mut application = self.0.application.borrow_mut();
        application.layers_mut().save(&ModelLayer::User, &*store).map_err(|e| e.0)
    }

    /// Shows the model in this language, from the translations given.
    pub fn set_language(&self, language: &str) -> Result<(), String> {
        self.0.application.borrow_mut().set_language(language).map_err(|e| e.0)?;
        self.touch();
        Ok(())
    }

    /// Registers how a list can be shown, for a view's `EditorType` in the model to pick by name,
    /// or `DefaultListEditor` on `Views` for all of them.
    pub fn register_list_editor(
        &self,
        name: impl Into<String>,
        editor: impl Fn(&crate::ListEditorRequest) -> ravenrs_widgets::AnyView + 'static,
    ) {
        self.0.list_editors.borrow_mut().insert(name.into(), Rc::new(editor));
    }

    /// Shows this list with the editor of this name, which stays in the person's own layer.
    pub fn choose_list_editor(&self, view: &str, editor: &str) -> Result<(), String> {
        self.0
            .application
            .borrow_mut()
            .set_user_setting(&["Views", view], "EditorType", editor)
            .map_err(|e| e.0)?;
        self.keep_settings()?;
        self.touch();
        Ok(())
    }

    pub(crate) fn list_editor(&self, name: &str) -> Option<crate::list_view::ListEditorFactory> {
        self.0.list_editors.borrow().get(name).cloned()
    }

    /// Keeps the content of attached files there, apart from the records. Without it, a file stays
    /// inside its record's field.
    pub fn use_attachment_store(&self, store: Rc<dyn AttachmentStore>) {
        *self.0.attachment_store.borrow_mut() = Some(store);
    }

    pub(crate) fn attachment_store(&self) -> Option<Rc<dyn AttachmentStore>> {
        self.0.attachment_store.borrow().clone()
    }

    pub(crate) fn set_popup_filter(&self, filter: Option<Criteria>) {
        *self.0.popup_filter.borrow_mut() = filter;
    }

    pub(crate) fn popup_filter(&self) -> Option<Criteria> {
        self.0.popup_filter.borrow().clone()
    }

    /// The keyboard left this field, so a form that checks on leaving can say what is wrong with it.
    pub(crate) fn left_field(&self, field: &str) {
        if !self.0.left.peek().contains(field) {
            self.0.left.update(|left| {
                left.insert(field.to_owned());
            });
        }
    }

    /// Opens on a new record with what this one holds, leaving out what its fields say stays
    /// behind. Lists that belong to it come along, and are kept with it.
    pub(crate) fn clone_record(&self, source: &Object) -> Result<(), String> {
        let entity = self.entity(source.entity()).ok_or_else(|| source.entity().to_owned())?;
        let view = self.detail_of(&entity)?;
        if self.0.frame.view().is_some_and(|v| v.kind == ViewKind::List) {
            self.0.history.update(|history| history.push(view.id.clone()));
        }
        self.show(view);
        let mut pending = Vec::new();
        let copy = self.copy_of(&entity, source, &mut pending)?;
        *self.0.pending.borrow_mut() = pending;
        self.0.form.set(Some(Rc::new(FormState::new(entity.clone(), copy, true))));
        self.update_position(&entity);
        Ok(())
    }

    /// A new record with what this one holds; what belongs to it goes in `pending`, pointing to it.
    fn copy_of(
        &self,
        entity: &EntityInfo,
        source: &Object,
        pending: &mut Vec<Object>,
    ) -> Result<Object, String> {
        let space = self.0.frame.space();
        let mut copy = space.borrow_mut().new_object(&entity.name).map_err(|e| e.to_string())?;
        let key_name = entity.key_field().map(|key| key.name.clone()).unwrap_or_default();
        let copied = entity.fields.iter().filter(|field| {
            !field.is_key
                && !field.non_cloneable
                && field.calculated.is_none()
                && !matches!(field.kind, FieldKind::Collection(_))
        });
        for field in copied {
            copy.set(&field.name, source.get(&field.name));
        }
        for field in entity.fields.iter().filter(|field| field.aggregated) {
            let FieldKind::Collection(target) = &field.kind else { continue };
            let Some(child) = self.entity(target) else { continue };
            let named = field.back_reference.as_deref();
            let Some(back) = crate::nested::back_reference(self, entity, target, named) else { continue };
            let filter = Criteria::property(&back).equals(Criteria::value(source.get(&key_name)));
            let found = space.borrow().query(target, &Query::all().filter(filter));
            for record in found.map_err(|e| e.to_string())? {
                let mut child_copy = self.copy_of(&child, &record, pending)?;
                child_copy.set(&back, copy.get(&key_name));
                pending.push(child_copy);
            }
        }
        Ok(copy)
    }

    /// What the person changed in the model, to keep between runs.
    pub fn user_settings(&self) -> Option<String> {
        let mut application = self.0.application.borrow_mut();
        application.layers_mut().difference(&ModelLayer::User).map(|diff| diff.to_json())
    }

    /// Puts this record on the form, as it is, without looking in storage.
    pub(crate) fn open_object(&self, entity: &EntityInfo, object: Object, is_new: bool) {
        match self.detail_of(entity) {
            Ok(view) => self.show(view),
            Err(error) => return self.0.error.set(Some(error)),
        }
        self.0.form.set(Some(Rc::new(FormState::new(entity.clone(), object, is_new))));
    }

    /// Changes a field of the form, as a screen of its own around the form would.
    pub fn set_form_value(&self, field: &str, value: Value) {
        if let Some(state) = self.0.form.peek().as_ref().and_then(|form| form.fields.get(field).cloned()) {
            state.set(value);
        }
    }

    /// The record on the form, if it meets its rules. If not, the form says what is wrong.
    pub(crate) fn check_form(&self) -> Option<(Object, bool)> {
        let form = self.0.form.peek().clone()?;
        let object = form.current();
        if !self.violations_of(&object, CONTEXT_SAVE).iter().any(Violation::is_error) {
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
        let today = Date::from_days(ravenrs_app::local_now().days);
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

    /// Goes up whenever records may have changed; reading it builds again when they do.
    pub fn revision(&self) -> u64 {
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
        let Some(form) = self.form() else { return Vec::new() };
        let found = |record: &Object| self.violations_of(record, CONTEXT_SAVE);
        if self.0.checking.get() || self.0.noting.get() {
            return found(&form.watched());
        }
        if !form.entity.checks_on_leave {
            return Vec::new();
        }
        let left = self.0.left.get();
        if left.is_empty() {
            return Vec::new();
        }
        // Only what the person already went through: what is still ahead is not an error yet.
        let mut all = found(&form.watched());
        all.retain(|violation| violation.field.as_ref().is_some_and(|field| left.contains(field)));
        all
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
    pub fn hand_file(&self, name: String, bytes: Vec<u8>, picker: &FilePicker) {
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
        self.0.full.set(None);
        self.0.error.set(None);
        self.0.checking.set(false);
        self.0.noting.set(false);
        self.0.pending.borrow_mut().clear();
        if !self.0.left.peek().is_empty() {
            self.0.left.set(BTreeSet::new());
        }
        self.0.frame.set_view_in(Some(view), (self.0.new_space)());
    }

    /// Shows a list, asking first if that would lose changes.
    pub fn open_view(&self, id: &str) {
        let (session, id) = (self.clone(), id.to_owned());
        self.leaving(move || session.show_view_now(&id));
    }

    /// Goes to where a navigation item points: a list, or a form with no list before it, on the
    /// record with this key or on a new one. Asks first if that would lose changes.
    pub(crate) fn open_navigation(&self, id: &str, key: Option<&str>) {
        let (session, id, key) = (self.clone(), id.to_owned(), key.map(str::to_owned));
        self.leaving(move || {
            let model = session.model();
            let Ok(view) = ViewInfo::from_model(&model, &id) else {
                return session.show_view_now(&id);
            };
            let Some(entity) = view.entity.as_deref().filter(|_| view.kind == ViewKind::Detail) else {
                return session.show_view_now(&id);
            };
            session.0.history.set(vec![id.clone()]);
            match (&key, session.entity(entity)) {
                (Some(key), Some(info)) => {
                    session.open_detail(entity, &crate::values::key_from_text(&info, key));
                }
                _ => session.open_new(entity),
            }
            session.touch();
        });
    }

    pub(crate) fn show_view_now(&self, id: &str) {
        let model = self.model();
        match ViewInfo::from_model(&model, id) {
            Ok(view) if view.kind == ViewKind::Detail => {
                self.0.history.set(vec![id.to_owned()]);
                match view.entity.clone() {
                    Some(entity) => self.open_new(&entity),
                    None => self.show(view),
                }
                self.touch();
            }
            Ok(view) => {
                self.0.form.set(None);
                self.0.history.set(vec![id.to_owned()]);
                let one_record =
                    view.entity.as_deref().and_then(|entity| self.entity(entity)).filter(|e| e.singleton);
                match one_record {
                    Some(entity) => self.open_the_record(&entity),
                    None => self.show(view),
                }
                self.touch();
            }
            Err(error) => self.0.error.set(Some(error.0)),
        }
    }

    /// The form of the one record of an entity that has only one, or of a new one if it has none.
    fn open_the_record(&self, entity: &EntityInfo) {
        let space = self.0.frame.space();
        let first = space.borrow().query(&entity.name, &Query::all()).ok().and_then(|all| {
            let name = entity.key_field()?.name.clone();
            all.first().map(|record| record.get(&name))
        });
        match first {
            Some(key) => self.open_detail(&entity.name, &key),
            None => self.open_new(&entity.name),
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

    /// A record by its key, in storage or from what gives the records no storage keeps.
    fn find_record(
        &self,
        entity: &EntityInfo,
        key: &Value,
    ) -> Result<Option<Object>, ravenrs_app::SpaceError> {
        if let Some(source) = self.record_source(&entity.name) {
            let name = entity.key_field().map(|field| field.name.clone()).unwrap_or_default();
            return Ok(source().into_iter().find(|object| object.get(&name) == *key));
        }
        self.0.frame.space().borrow().find(&entity.name, key)
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
        let found = self.find_record(&entity, key);
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
        // What no storage keeps starts from its declaration.
        let made = if entity.transient {
            Ok(entity.new_object())
        } else {
            self.0.frame.space().borrow_mut().new_object(entity_name)
        };
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
        self.save_then(|_| {})
    }

    /// Keeps what the form holds and then runs `then`. What only warns asks first, and so does a
    /// field someone else changed too, so `then` may run later, when the person answers, or never.
    pub(crate) fn save_then(&self, then: impl Fn(&Self) + 'static) -> Result<(), String> {
        let Some(form) = self.0.form.peek().clone() else { return Ok(()) };
        let object = form.current();
        let found = self.violations_of(&object, CONTEXT_SAVE);
        let errors: Vec<String> = found.iter().filter(|v| v.is_error()).map(|v| v.message.clone()).collect();
        if !errors.is_empty() {
            self.0.error.set(None);
            self.0.checking.set(true);
            return Err(errors.join("; "));
        }
        self.0.checking.set(false);
        let then: Rc<dyn Fn(&Self)> = Rc::new(then);
        let warnings: Vec<String> =
            found.iter().filter(|v| v.severity == Severity::Warning).map(|v| v.message.clone()).collect();
        if warnings.is_empty() {
            return self.save_merged(&then);
        }
        let session = self.clone();
        let words = self.words();
        self.0.confirm.ask(
            words.warning_question.clone(),
            warnings.join("\n"),
            words.warning_answer.clone(),
            move || {
                // What fails goes on the screen.
                let _ = session.save_merged(&then);
            },
        );
        Ok(())
    }

    /// Puts together what the form changed and what someone else kept since it opened. Different
    /// fields both stand; the same field asks first.
    fn save_merged(&self, then: &Rc<dyn Fn(&Self)>) -> Result<(), String> {
        let Some(form) = self.0.form.peek().clone() else { return Ok(()) };
        let object = form.current();
        if form.is_new || form.entity.transient {
            return self.keep(&form, object, then);
        }
        let key = form.entity.key_field().map_or(Value::Null, |field| object.get(&field.name));
        let stored = self.0.frame.space().borrow().find(&form.entity.name, &key);
        let stored = match stored {
            Ok(stored) => stored,
            Err(error) => {
                let message = error.to_string();
                self.0.error.set(Some(message.clone()));
                return Err(message);
            }
        };
        let Some(stored) = stored else {
            let message = self.words().record_gone.clone();
            self.0.error.set(Some(message.clone()));
            return Err(message);
        };
        let merged = merge_changes(&form.entity, &form.original, &object, &stored);
        if merged.conflicts.is_empty() {
            return self.keep(&form, merged.record, then);
        }
        let names: Vec<&str> = merged
            .conflicts
            .iter()
            .filter_map(|name| form.entity.field_named(name))
            .map(|field| field.caption.as_str())
            .collect();
        let (session, then, record) = (self.clone(), then.clone(), merged.record);
        let (words, kept_form) = (self.words(), form.clone());
        self.0.confirm.ask(
            words.conflict_question.clone(),
            words.conflict_detail.replace("{0}", &names.join(", ")),
            words.conflict_answer.clone(),
            move || {
                // What fails goes on the screen.
                let _ = session.keep(&kept_form, record.clone(), &then);
            },
        );
        Ok(())
    }

    /// Writes the record, with what comes along with it.
    fn keep(&self, form: &Rc<FormState>, object: Object, then: &Rc<dyn Fn(&Self)>) -> Result<(), String> {
        let has_notes = self.violations_of(&object, CONTEXT_SAVE).iter().any(|v| !v.is_error());
        self.0.noting.set(has_notes);
        if form.entity.transient {
            self.0.error.set(None);
            self.0.form.set(Some(Rc::new(FormState::new(form.entity.clone(), object, false))));
            self.touch();
            then(self);
            return Ok(());
        }
        let space = self.0.frame.space();
        let done = {
            let mut space = space.borrow_mut();
            let mut kept =
                if form.is_new { space.insert(object.clone()) } else { space.save(object.clone()) };
            // After the record, which they point to.
            for record in self.0.pending.borrow_mut().drain(..) {
                kept = kept.and_then(|()| space.insert(record));
            }
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
                then(self);
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
        let found: Vec<Violation> =
            objects.iter().flat_map(|object| self.violations_of(object, CONTEXT_DELETE)).collect();
        let refused: Vec<String> = found.iter().filter(|v| v.is_error()).map(|v| v.message.clone()).collect();
        if !refused.is_empty() {
            return Err(refused.join("; "));
        }
        let warnings: Vec<String> =
            found.iter().filter(|v| v.severity == Severity::Warning).map(|v| v.message.clone()).collect();
        if warnings.is_empty() {
            return self.delete_now(objects);
        }
        let (session, objects, words) = (self.clone(), objects.to_vec(), self.words());
        self.0.confirm.ask(
            words.delete_question.clone(),
            warnings.join("\n"),
            words.delete.clone(),
            move || {
                // What fails goes on the screen.
                let _ = session.delete_now(&objects);
            },
        );
        Ok(())
    }

    fn delete_now(&self, objects: &[Object]) -> Result<(), String> {
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
        let model = self.model();
        model.get("NavigationItems")?.in_order().into_iter().find_map(|group| {
            if let Some(view) = group.text("View") {
                return self.can_open(None, &group.id, view).then(|| view.to_owned());
            }
            group.in_order().into_iter().find_map(|item| {
                let view = item.text("View")?;
                self.can_open(Some(&group.id), &item.id, view).then(|| view.to_owned())
            })
        })
    }

    /// Runs an action from a screen: asks its question first, and does what it asks for next.
    pub fn run_action(&self, action: &Action, input: ActionInput) {
        let Some(question) = action.confirmation_text().map(str::to_owned) else {
            self.execute_after_warnings(action, input);
            return;
        };
        let (session, kept) = (self.clone(), action.clone());
        let detail = action.confirmation_detail().unwrap_or(&self.words().delete_detail).to_owned();
        let answer = action.confirmation_answer().unwrap_or(action.caption()).to_owned();
        self.0.confirm.ask(question, detail, answer, move || {
            session.execute_after_warnings(&kept, input.clone());
        });
    }

    /// Runs an action, asking first about what its contexts only warn of.
    fn execute_after_warnings(&self, action: &Action, input: ActionInput) {
        let selected = self.0.frame.selected();
        let records = match (self.form(), selected.is_empty()) {
            (Some(form), true) => vec![form.current()],
            _ => selected,
        };
        let warnings: Vec<String> = records
            .iter()
            .flat_map(|record| {
                action.contexts().iter().flat_map(move |context| self.violations_of(record, context))
            })
            .filter(|violation| violation.severity == Severity::Warning)
            .map(|violation| violation.message)
            .collect();
        if warnings.is_empty() {
            return self.execute(action.id(), input);
        }
        let (session, id, words) = (self.clone(), action.id().to_owned(), self.words());
        self.0.confirm.ask(
            words.warning_question.clone(),
            warnings.join("\n"),
            words.warning_answer.clone(),
            move || session.execute(&id, input.clone()),
        );
    }

    /// An action changed records: what is selected reads again, so actions go by what they are now,
    /// and a form with nothing typed in shows what they are now. What someone typed stays, and
    /// saving puts it together with what changed.
    fn reread_after_action(&self) {
        let frame = &self.0.frame;
        let fresh: Vec<Object> = frame
            .selected()
            .into_iter()
            .filter_map(|record| {
                let entity = self.entity(record.entity())?;
                let key = record.get(&entity.key_field()?.name);
                frame.space().borrow().find(&entity.name, &key).ok().flatten()
            })
            .collect();
        frame.set_selected(fresh);
        self.touch();
        let form = self.0.form.peek().clone();
        if let Some(form) = form.filter(|form| !form.is_new && !form.is_dirty())
            && let Some(field) = form.entity.key_field()
        {
            self.open_detail(&form.entity.name, &form.original.get(&field.name));
        }
    }

    fn execute(&self, id: &str, input: ActionInput) {
        match self.0.frame.execute(id, input) {
            Ok(ActionResult::OpenUrl(address)) => {
                let handler = self.0.on_url.borrow().clone();
                if let Some(handler) = handler {
                    handler(address);
                }
            }
            Ok(ActionResult::RecordsChanged) => self.reread_after_action(),
            Ok(_) => {}
            // The form already says what is wrong, on each field.
            Err(_) if *self.0.checking.peek() => {}
            Err(error) => self.0.error.set(Some(error.to_string())),
        }
    }
}
