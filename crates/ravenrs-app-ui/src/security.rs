//! Signing in and managing passwords, on top of the permissions in `ravenrs-app`. What is protected
//! is protected under this, in the secured space; these screens are the way in.

use std::cell::RefCell;
use std::rc::Rc;

use ravenrs_app::{
    Action, ActionCategory, AuditSettings, AuditTrail, AuditedObjectSpace, AuthError, Authenticator,
    Credentials, EntityInfo, Identity, Message, MessageKind, ModelError, Module, ObjectSpace, PasswordError,
    Role, RoleMerge, SECURITY_USER, SecuredObjectSpace, SecurityContext, Selection, SpaceMaker,
    UserDirectory, Value, ViewController, audit_entity, security_entities,
};
use ravenrs_layout::{Align, Justify};
use ravenrs_reactive::{Binding, State};
use ravenrs_style::{TextStyle, Weight};
use ravenrs_widgets::{
    Alert, AnyView, BuildCx, Button, Card, FormField, HStack, Keyed, Localized, NodeId, Text, TextField,
    Tone, VStack, View, ViewExt,
};

use crate::model_editor::ModelEditor;
use crate::session::Words;
use crate::{App, Language, Session};

/// The id of the action that gives someone a new password.
pub(crate) const SET_PASSWORD: &str = "SetPassword";
/// The control of the model editor, on its own view.
pub(crate) const MODEL_EDITOR: &str = "ModelEditor";
pub(crate) const MODEL_EDITOR_VIEW: &str = "ModelEditor_DashboardView";
const CARD_WIDTH: f32 = 380.0;
/// Their words in Brazilian Portuguese.
const PT_BR: &str = include_str!("security_pt_br.json");

fn message_of(words: &Words, error: &PasswordError) -> String {
    match error {
        PasswordError::TooShort => words.password_too_short.clone(),
        PasswordError::WrongCurrent => words.password_wrong_current.clone(),
        PasswordError::SameAsCurrent => words.password_same.clone(),
        other => other.to_string(),
    }
}

fn text_binding(state: &State<String>) -> Binding<String> {
    let (read, write) = (state.clone(), state.clone());
    Binding::new(move || read.get(), move |value| write.set(value))
}

type Submit = Rc<dyn Fn(Option<&str>, &str) -> Result<(), PasswordError>>;

/// Asks for a new password, twice, and for the current one when that is what it takes.
#[derive(Clone)]
struct PasswordForm {
    words: Words,
    current: Option<State<String>>,
    new: State<String>,
    repeated: State<String>,
    error: State<Option<String>>,
    submit: Submit,
    done: Rc<dyn Fn()>,
}

impl PasswordForm {
    fn new(words: Words, asks_current: bool, submit: Submit, done: Rc<dyn Fn()>) -> Self {
        Self {
            words,
            current: asks_current.then(|| State::new(String::new())),
            new: State::new(String::new()),
            repeated: State::new(String::new()),
            error: State::new(None),
            submit,
            done,
        }
    }

    fn send(&self) {
        let (new, repeated) = (self.new.peek().clone(), self.repeated.peek().clone());
        if new != repeated {
            self.error.set(Some(self.words.passwords_differ.clone()));
            return;
        }
        let current = self.current.as_ref().map(|state| state.peek().clone());
        match (self.submit)(current.as_deref(), &new) {
            Ok(()) => {
                for state in
                    [Some(&self.new), Some(&self.repeated), self.current.as_ref()].into_iter().flatten()
                {
                    state.set(String::new());
                }
                self.error.set(None);
                (self.done)();
            }
            Err(error) => self.error.set(Some(message_of(&self.words, &error))),
        }
    }
}

impl View for PasswordForm {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let words = &self.words;
        let sending = self.clone();
        let current: Option<AnyView> = self.current.as_ref().map(|state| {
            Box::new(FormField::new(
                words.current_password.clone(),
                TextField::new(text_binding(state)).secure().autofocus(),
            )) as AnyView
        });
        let form = self.clone();
        let content = VStack::new((
            self.error.get().map(|error| Alert::new(words.change_password.clone(), error).tone(Tone::Danger)),
            current,
            FormField::new(words.new_password.clone(), TextField::new(text_binding(&self.new)).secure()),
            FormField::new(
                words.repeat_password.clone(),
                TextField::new(text_binding(&self.repeated)).secure().on_submit(move |_| form.send()),
            ),
            Button::new(words.save_password.clone()).on_click(move || sending.send()),
        ))
        .spacing(12.0)
        .align(Align::Stretch);
        cx.child(&content)
    }
}

/// Users, roles, the audit trail and giving someone a password.
#[derive(Debug)]
pub(crate) struct SecurityModule {
    words: Words,
    audited: bool,
}

impl Module for SecurityModule {
    fn name(&self) -> &str {
        "Security"
    }

    fn entities(&self) -> Vec<EntityInfo> {
        let mut entities = security_entities();
        if self.audited {
            entities.push(audit_entity());
        }
        entities
    }

    fn view_controllers(&self) -> Vec<Box<dyn ViewController>> {
        vec![Box::new(PasswordsController(self.words.clone()))]
    }
}

struct PasswordsController(Words);

impl ViewController for PasswordsController {
    fn id(&self) -> &str {
        "Passwords"
    }

    fn actions(&self) -> Vec<Action> {
        let words = self.0.clone();
        vec![
            Action::simple(SET_PASSWORD, words.set_password.clone())
                .category(ActionCategory::Edit)
                .target_entity(SECURITY_USER)
                .selection(Selection::RequireSingle)
                .icon("key-round")
                .on_execute(move |cx| {
                    let session = Session::of(cx).ok_or("this action needs the application's screens")?;
                    let user = cx.selected.first().map_or(Value::Null, |user| user.get("id"));
                    show_set_password(&session, user, words.clone());
                    Ok(())
                }),
        ]
    }
}

fn show_set_password(session: &Session, user: Value, words: Words) {
    let (context, shown) = (session.frame().security(), session.clone());
    let submit: Submit = Rc::new(move |_, password| match &context {
        Some(context) => context.set_password_of(&user, password),
        None => Err(PasswordError::NotAllowed),
    });
    let told = words.password_saved.clone();
    let done: Rc<dyn Fn()> = Rc::new(move || {
        shown.close_presented();
        shown.show_message(Message::new(told.clone(), MessageKind::Success));
        shown.records_changed();
    });
    let form = PasswordForm::new(words.clone(), false, submit, done);
    session.present(words.set_password, move || Box::new(form.clone()) as AnyView);
}

/// Shows the person's own way of changing their password.
pub(crate) fn show_change_password(session: &Session) {
    let words = session.words().clone();
    let (context, shown) = (session.frame().security(), session.clone());
    let submit: Submit = Rc::new(move |current, password| match &context {
        Some(context) => context.change_own_password(current.unwrap_or(""), password),
        None => Err(PasswordError::NotAllowed),
    });
    let told = words.password_saved.clone();
    let done: Rc<dyn Fn()> = Rc::new(move || {
        shown.close_presented();
        shown.show_message(Message::new(told.clone(), MessageKind::Success));
    });
    let form = PasswordForm::new(words.clone(), true, submit, done);
    session.present(words.change_password, move || Box::new(form.clone()) as AnyView);
}

type MakeApp = Rc<dyn Fn(&Identity) -> App>;
type Seed = Box<dyn FnOnce(&mut dyn ObjectSpace) -> Result<(), String>>;

/// An application people sign in to, each seeing what their roles allow.
///
/// Storage has to know [`security_entities`], and [`audit_entity`] with an audit trail. The
/// application is made anew for each person, once they have signed in, so nothing of its model is
/// read before that, and every view goes through a [`SecuredObjectSpace`].
///
/// ```
/// use ravenrs_app::{EntityInfo, FieldInfo, FieldKind, MemoryDatabase, security_entities};
/// use ravenrs_app_ui::{App, SecuredApp};
///
/// let account = EntityInfo::new("Account")
///     .navigation("Records")
///     .field(FieldInfo::new("id", FieldKind::Integer).key())
///     .field(FieldInfo::new("name", FieldKind::Text));
/// let mut entities = security_entities();
/// entities.push(account.clone());
/// let database = MemoryDatabase::new(entities)?;
/// let window = SecuredApp::new("Accounts", move || database.space(), move |_person| {
///     App::new("Accounts").entity_info(account.clone())
/// })
/// .build()?;
/// # let _ = window;
/// # Ok::<(), ravenrs_app::ModelError>(())
/// ```
pub struct SecuredApp {
    space: SpaceMaker,
    application: MakeApp,
    language: Language,
    roles: Vec<Role>,
    merge: RoleMerge,
    audit: Option<AuditSettings>,
    authenticator: Option<Rc<dyn Authenticator>>,
    seed: Option<Seed>,
    title: String,
}

impl std::fmt::Debug for SecuredApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecuredApp").field("title", &self.title).finish_non_exhaustive()
    }
}

impl SecuredApp {
    /// Over what `space` makes, with no permissions of its own. `application` says what the
    /// application has, for whoever signed in; its storage and seed are replaced by what is here.
    pub fn new<S: ObjectSpace + 'static>(
        title: impl Into<String>,
        space: impl Fn() -> S + 'static,
        application: impl Fn(&Identity) -> App + 'static,
    ) -> Self {
        Self {
            space: Rc::new(move || Box::new(space()) as Box<dyn ObjectSpace>),
            application: Rc::new(application),
            language: Language::default(),
            roles: Vec::new(),
            merge: RoleMerge::default(),
            audit: None,
            authenticator: None,
            seed: None,
            title: title.into(),
        }
    }

    /// The language of its screens.
    pub fn language(mut self, language: Language) -> Self {
        self.language = language;
        self
    }

    /// A role declared in code. Its record is created if missing, and it adds to what that says.
    pub fn role(mut self, role: Role) -> Self {
        self.roles.push(role);
        self
    }

    /// How the roles of one person come together.
    pub fn merge(mut self, merge: RoleMerge) -> Self {
        self.merge = merge;
        self
    }

    /// Writes who changed what to the audit trail, in the same commit as each change.
    pub fn audit(mut self, settings: AuditSettings) -> Self {
        self.audit = Some(settings);
        self
    }

    /// Another way of signing in than the passwords it keeps, as with an outside provider.
    pub fn authenticator(mut self, authenticator: impl Authenticator + 'static) -> Self {
        self.authenticator = Some(Rc::new(authenticator));
        self
    }

    /// What is in its records when it opens, with no permissions in the way, once per run.
    pub fn seed(mut self, seed: impl FnOnce(&mut dyn ObjectSpace) -> Result<(), String> + 'static) -> Self {
        self.seed = Some(Box::new(seed));
        self
    }

    /// Puts it together, or says what is wrong with how it was declared.
    pub fn build(self) -> Result<AnyView, ModelError> {
        let wrong = |error: ravenrs_app::SpaceError| ModelError(error.to_string());
        let known = (self.space)().entities();
        for needed in security_entities().into_iter().chain(self.audit.as_ref().map(|_| audit_entity())) {
            if !known.iter().any(|entity| entity.name == needed.name) {
                return Err(ModelError(format!("storage has to know the entity {}", needed.name)));
            }
        }
        let directory = UserDirectory::new(self.space.clone(), self.roles.clone()).map_err(wrong)?;
        if let Some(seed) = self.seed {
            let mut space = (self.space)();
            seed(&mut *space).map_err(ModelError)?;
            space.commit().map_err(wrong)?;
        }
        let stage = if directory.is_empty().map_err(wrong)? { Stage::FirstUser } else { Stage::SignIn };
        let words = self.language.words();
        let authenticator: Rc<dyn Authenticator> = match self.authenticator {
            Some(authenticator) => authenticator,
            None => Rc::new(directory.clone()),
        };
        let gate = Gate(Rc::new(GateInner {
            title: self.title,
            words,
            language: self.language,
            space: self.space,
            application: self.application,
            merge: self.merge,
            audit: self.audit,
            directory,
            authenticator,
            entities: known,
            stage: State::new(stage),
            name: State::new(String::new()),
            password: State::new(String::new()),
            repeated: State::new(String::new()),
            error: State::new(None),
            entered: RefCell::new(0),
        }));
        Ok(Box::new(Localized::new(gate.0.language.labels(), gate)))
    }

    /// Puts it together, showing on the screen what is wrong with how it was declared.
    pub fn into_view(self) -> AnyView {
        self.build().unwrap_or_else(|error| Box::new(Alert::new("RavenRS", error.0).tone(Tone::Danger)))
    }
}

#[derive(Clone)]
enum Stage {
    /// Nobody can sign in yet.
    FirstUser,
    SignIn,
    /// They signed in with a password they were given.
    NewPassword(Rc<PasswordForm>),
    /// The application, for this person.
    Inside(Rc<AnyView>, u64),
}

struct GateInner {
    title: String,
    words: Words,
    language: Language,
    space: SpaceMaker,
    application: MakeApp,
    merge: RoleMerge,
    audit: Option<AuditSettings>,
    directory: UserDirectory,
    authenticator: Rc<dyn Authenticator>,
    entities: Vec<EntityInfo>,
    stage: State<Stage>,
    name: State<String>,
    password: State<String>,
    repeated: State<String>,
    error: State<Option<String>>,
    entered: RefCell<u64>,
}

/// What stands between people and the application.
#[derive(Clone)]
struct Gate(Rc<GateInner>);

impl Gate {
    fn forget_typed(&self) {
        self.0.password.set(String::new());
        self.0.repeated.set(String::new());
    }

    fn sign_in(&self) {
        let inner = &self.0;
        let credentials = Credentials::new(inner.name.peek().clone(), inner.password.peek().clone());
        let result = inner.authenticator.authenticate(&credentials);
        self.forget_typed();
        match result {
            Ok(identity) => self.enter(identity),
            Err(AuthError::Rejected) => inner.error.set(Some(inner.words.sign_in_failed.clone())),
            // What went wrong in storage is not for whoever is at the door.
            Err(_) => inner.error.set(Some(inner.words.sign_in_unavailable.clone())),
        }
    }

    fn create_first(&self) {
        let inner = &self.0;
        let (password, repeated) = (inner.password.peek().clone(), inner.repeated.peek().clone());
        if password != repeated {
            inner.error.set(Some(inner.words.passwords_differ.clone()));
            return;
        }
        let name = inner.name.peek().clone();
        match inner.directory.create_first_administrator(&name, &password) {
            Ok(identity) => {
                self.forget_typed();
                self.enter(identity);
            }
            Err(error) => inner.error.set(Some(message_of(&inner.words, &error))),
        }
    }

    fn enter(&self, identity: Identity) {
        let inner = &self.0;
        inner.error.set(None);
        let must_change = identity.must_change_password;
        let audit_author = identity.name.clone();
        let mut context =
            SecurityContext::new(identity, inner.directory.clone(), inner.entities.clone(), inner.merge)
                .with_denied_message(inner.words.access_denied.clone());
        let trail = inner.audit.clone().map(|settings| {
            let trail = Rc::new(AuditTrail::new(settings, move || audit_author.clone()));
            trail.write_with(inner.space.clone());
            trail
        });
        if let Some(trail) = &trail {
            context = context.with_audit_trail(trail.clone());
        }
        let context = Rc::new(context);
        if must_change {
            let (changing, gate) = (context.clone(), self.clone());
            let submit: Submit =
                Rc::new(move |current, new| changing.change_own_password(current.unwrap_or(""), new));
            let done: Rc<dyn Fn()> = Rc::new(move || gate.open_application(context.clone(), trail.clone()));
            let form = PasswordForm::new(inner.words.clone(), true, submit, done);
            inner.stage.set(Stage::NewPassword(Rc::new(form)));
        } else {
            self.open_application(context, trail);
        }
    }

    fn open_application(&self, context: Rc<SecurityContext>, trail: Option<Rc<AuditTrail>>) {
        let inner = &self.0;
        let identity = context.identity();
        let (raw, secured_context, secured_trail) = (inner.space.clone(), context.clone(), trail.clone());
        let storage = move || {
            // Each view reads who they are again, so what someone else changed about them counts.
            // If that fails they keep what they had, and storage is failing for the view too.
            let _ = secured_context.reload();
            let under: Box<dyn ObjectSpace> = match &secured_trail {
                Some(trail) => Box::new(AuditedObjectSpace::new(raw(), trail.clone())),
                None => raw(),
            };
            SecuredObjectSpace::new(under, secured_context.clone())
        };
        let (gate, words) = (self.clone(), inner.words.clone());
        let audited = trail.is_some();
        let app = (inner.application)(&identity)
            .language(inner.language)
            .module(SecurityModule { words: words.clone(), audited })
            .translation("pt-BR", PT_BR)
            .dashboard(
                ravenrs_app::DashboardInfo::new(MODEL_EDITOR, words.model_editor.clone())
                    .navigation("Administration")
                    .icon("settings")
                    .control(words.model_editor.clone(), MODEL_EDITOR),
            )
            .storage(storage)
            .settings_over(inner.space.clone(), &inner.entities)
            .secured(context, inner.space.clone())
            .signed_in(identity.name.clone(), move || gate.sign_out());
        match app.build() {
            Ok(view) => {
                *inner.entered.borrow_mut() += 1;
                inner.stage.set(Stage::Inside(Rc::new(view), *inner.entered.borrow()));
            }
            Err(error) => inner.error.set(Some(error.0)),
        }
    }

    fn sign_out(&self) {
        self.forget_typed();
        self.0.name.set(String::new());
        self.0.error.set(None);
        self.0.stage.set(Stage::SignIn);
    }

    fn title(&self, detail: Option<&str>) -> AnyView {
        Box::new(
            VStack::new((
                Text::new(self.0.title.clone()).style(TextStyle::Title).font_weight(Weight::SEMIBOLD),
                detail.map(|text| Text::new(text.to_owned()).style(TextStyle::Caption)),
            ))
            .spacing(4.0)
            .align(Align::Stretch),
        )
    }

    fn sign_in_form(&self) -> AnyView {
        let (words, inner) = (&self.0.words, &self.0);
        let (by_key, by_button) = (self.clone(), self.clone());
        Box::new(
            VStack::new((
                self.title(None),
                inner.error.get().map(|error| Alert::message(error).tone(Tone::Danger)),
                FormField::new(
                    words.user_name.clone(),
                    TextField::new(text_binding(&inner.name)).autofocus(),
                ),
                FormField::new(
                    words.password.clone(),
                    TextField::new(text_binding(&inner.password))
                        .secure()
                        .on_submit(move |_| by_key.sign_in()),
                ),
                Button::new(words.sign_in.clone()).on_click(move || by_button.sign_in()).full_width(),
            ))
            .spacing(12.0)
            .align(Align::Stretch),
        )
    }

    fn first_user_form(&self) -> AnyView {
        let (words, inner) = (&self.0.words, &self.0);
        let (by_key, by_button) = (self.clone(), self.clone());
        Box::new(
            VStack::new((
                self.title(Some(&words.first_user)),
                inner
                    .error
                    .get()
                    .map(|error| Alert::new(words.create_user.clone(), error).tone(Tone::Danger)),
                FormField::new(
                    words.user_name.clone(),
                    TextField::new(text_binding(&inner.name)).autofocus(),
                ),
                FormField::new(
                    words.new_password.clone(),
                    TextField::new(text_binding(&inner.password)).secure(),
                ),
                FormField::new(
                    words.repeat_password.clone(),
                    TextField::new(text_binding(&inner.repeated))
                        .secure()
                        .on_submit(move |_| by_key.create_first()),
                ),
                Button::new(words.create_user.clone()).on_click(move || by_button.create_first()),
            ))
            .spacing(12.0)
            .align(Align::Stretch),
        )
    }

    fn new_password_form(&self, form: &PasswordForm) -> AnyView {
        Box::new(
            VStack::new((self.title(Some(&self.0.words.must_change_password)), form.clone()))
                .spacing(12.0)
                .align(Align::Stretch),
        )
    }
}

impl View for Gate {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let form = match self.0.stage.get() {
            Stage::Inside(view, entered) => return cx.child(&Keyed::new(entered, SharedView(view))),
            Stage::FirstUser => self.first_user_form(),
            Stage::SignIn => self.sign_in_form(),
            Stage::NewPassword(form) => self.new_password_form(&form),
        };
        let centered = HStack::new((Card::new(form).padding(24.0).width(CARD_WIDTH),))
            .justify(Justify::Center)
            .align(Align::Center)
            .grow(1.0);
        cx.child(&centered)
    }
}

/// An application already put together, for the gate to keep.
struct SharedView(Rc<AnyView>);

impl View for SharedView {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        cx.child(&**self.0)
    }
}

/// What the model editor's control is made from. It keeps what is being edited between builds.
pub(crate) fn model_editor(session: &Session) -> impl Fn() -> AnyView + 'static {
    let editor = Rc::new(ModelEditor::new(session.clone()));
    move || Box::new(SharedEditor(editor.clone()))
}

struct SharedEditor(Rc<ModelEditor>);

impl View for SharedEditor {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        self.0.build(cx)
    }
}
