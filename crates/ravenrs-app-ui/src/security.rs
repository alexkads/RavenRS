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
use ravenrs_reactive::State;
use ravenrs_style::{TextStyle, Weight};
use ravenrs_widgets::{
    Alert, AnyView, BuildCx, Button, ButtonStyle, Card, FormField, HStack, IconName, Keyed, Localized,
    NodeId, Text, TextField, Tone, VStack, View, ViewExt,
};

use crate::model_editor::MODEL_EDITOR;
use crate::password::{PasswordForm, Submit, message_of, text_binding};
use crate::session::Words;
use crate::{App, Language, Session};

/// The id of the action that gives someone a new password.
pub(crate) const SET_PASSWORD: &str = "SetPassword";
const CARD_WIDTH: f32 = 380.0;

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

type MakeApp = Rc<dyn Fn(&Identity) -> App>;
type Seed = Box<dyn FnOnce(&mut dyn ObjectSpace) -> Result<(), String>>;

/// A button on the screen of signing in besides Sign in, such as going back to choosing the
/// organization. Pressing it forgets what was typed and any message, signs nobody in and carries
/// nothing over, then runs its handler.
#[derive(Clone)]
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct SignInAction {
    label: String,
    icon: Option<IconName>,
    on_click: Rc<dyn Fn()>,
}

impl std::fmt::Debug for SignInAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SignInAction").field("label", &self.label).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl SignInAction {
    /// A button saying `label` that runs `on_click`.
    pub fn new(label: impl Into<String>, on_click: impl Fn() + 'static) -> Self {
        Self { label: label.into(), icon: None, on_click: Rc::new(on_click) }
    }

    /// With this icon before its words.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }
}

/// A name and password filled in on the screen of signing in, for a demonstration. Nobody gets in
/// until Sign in is pressed and they are checked as any other.
#[derive(Clone)]
struct DemoCredentials {
    name: String,
    password: String,
}

/// How the screen of signing in presents itself.
#[derive(Clone, Default)]
struct Door {
    description: Option<String>,
    notice: Option<String>,
    actions: Vec<SignInAction>,
    demo: Option<DemoCredentials>,
}

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
#[cfg_attr(feature = "live", ravenrs_live::expose(part = "fields"))]
pub struct SecuredApp {
    space: SpaceMaker,
    application: MakeApp,
    language: Language,
    texts: crate::app::TextPacks,
    roles: Vec<Role>,
    merge: RoleMerge,
    audit: Option<AuditSettings>,
    authenticator: Option<Rc<dyn Authenticator>>,
    seed: Option<Seed>,
    title: String,
    door: Door,
}

impl std::fmt::Debug for SecuredApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecuredApp").field("title", &self.title).finish_non_exhaustive()
    }
}

#[cfg_attr(feature = "live", ravenrs_live::expose)]
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
            texts: Vec::new(),
            roles: Vec::new(),
            merge: RoleMerge::default(),
            audit: None,
            authenticator: None,
            seed: None,
            title: title.into(),
            door: Door::default(),
        }
    }

    /// A line under the title of the screen of signing in, such as which organization this is.
    pub fn description(mut self, text: impl Into<String>) -> Self {
        self.door.description = Some(text.into());
        self
    }

    /// A notice on the screen of signing in, such as saying it is a demonstration.
    pub fn notice(mut self, text: impl Into<String>) -> Self {
        self.door.notice = Some(text.into());
        self
    }

    /// A button besides Sign in, such as going back to choosing the organization. It shows while
    /// signing in, creating the first administrator and choosing a new password. On the last, the
    /// person signed in is left behind: nothing of them goes on.
    pub fn sign_in_action(mut self, action: SignInAction) -> Self {
        self.door.actions.push(action);
        self
    }

    /// Fills in this name and password on the screen of signing in, for a demonstration. Off unless
    /// asked for. They can be changed to try someone else, the password stays hidden, and Sign in
    /// still checks them as any other, with the same permissions, audit trail and rules.
    ///
    /// They are filled in when the screen opens, after signing out and after a [`SignInAction`]. A
    /// wrong try forgets the password, as always; it does not come back until one of those. Use
    /// only made-up people of a demonstration: they are in the program for anyone to read.
    pub fn demo_credentials(mut self, name: impl Into<String>, password: impl Into<String>) -> Self {
        self.door.demo = Some(DemoCredentials { name: name.into(), password: password.into() });
        self
    }

    /// The language of its screens.
    pub fn language(mut self, language: Language) -> Self {
        self.language = language;
        self
    }

    /// Adds or replaces words, as [`App::texts`] does, for the screen of signing in and for the
    /// application of each person.
    pub fn texts(mut self, language: impl Into<String>, json: impl Into<String>) -> Self {
        self.texts.push((language.into(), json.into()));
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
        let texts = crate::app::texts_of(self.language, &self.texts)?;
        let words = Words::from_texts(&texts);
        let authenticator: Rc<dyn Authenticator> = match self.authenticator {
            Some(authenticator) => authenticator,
            None => Rc::new(directory.clone()),
        };
        let gate = Gate(Rc::new(GateInner {
            title: self.title,
            words,
            language: self.language,
            texts: self.texts,
            labels: ravenrs_widgets::Labels::from_texts(&texts),
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
            door_theme: std::cell::Cell::new(None),
            door: self.door,
        }));
        gate.fill_demo();
        Ok(Box::new(Localized::new(gate.0.labels.clone(), gate)))
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
    door_theme: std::cell::Cell<Option<ravenrs_style::Theme>>,
    title: String,
    words: Words,
    language: Language,
    texts: crate::app::TextPacks,
    labels: ravenrs_widgets::Labels,
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
    door: Door,
}

/// What stands between people and the application.
#[derive(Clone)]
struct Gate(Rc<GateInner>);

#[cfg_attr(feature = "live", ravenrs_live::expose)]
impl Gate {
    fn forget_typed(&self) {
        self.0.password.set(String::new());
        self.0.repeated.set(String::new());
    }

    /// The name and password of a demonstration, where someone signs in.
    fn fill_demo(&self) {
        let inner = &self.0;
        if let Some(demo) = &inner.door.demo
            && matches!(*inner.stage.peek(), Stage::SignIn)
        {
            inner.name.set(demo.name.clone());
            inner.password.set(demo.password.clone());
        }
    }

    /// Leaves the door: what was typed and said goes, and whoever was halfway in is left behind.
    fn leave_by(&self, action: &SignInAction) {
        let inner = &self.0;
        self.forget_typed();
        inner.name.set(String::new());
        inner.error.set(None);
        let first = inner.directory.is_empty().unwrap_or(false);
        inner.stage.set(if first { Stage::FirstUser } else { Stage::SignIn });
        self.fill_demo();
        (action.on_click)();
    }

    /// The door's text and its buttons besides the main one.
    fn door_parts(&self) -> (Option<AnyView>, Option<AnyView>) {
        let door = &self.0.door;
        let notice: Option<AnyView> =
            door.notice.clone().map(|text| Box::new(Alert::message(text).tone(Tone::Accent)) as AnyView);
        let actions: Option<AnyView> = (!door.actions.is_empty()).then(|| {
            let buttons: Vec<AnyView> = door
                .actions
                .iter()
                .map(|action| {
                    let (gate, pressed) = (self.clone(), action.clone());
                    let button = Button::new(action.label.clone())
                        .style(ButtonStyle::Ghost)
                        .full_width()
                        .on_click(move || gate.leave_by(&pressed));
                    Box::new(match action.icon {
                        Some(icon) => button.icon(icon),
                        None => button,
                    }) as AnyView
                })
                .collect();
            Box::new(VStack::new(buttons).spacing(4.0).align(Align::Stretch)) as AnyView
        });
        (notice, actions)
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
        let mut app = (inner.application)(&identity).language(inner.language);
        for (language, json) in &inner.texts {
            app = app.texts(language.clone(), json.clone());
        }
        let app = app
            .module(SecurityModule { words: words.clone(), audited })
            .translation("pt-BR", ravenrs_app_i18n::model::SECURITY_PT_BR)
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
        self.fill_demo();
    }

    fn title(&self, detail: Option<&str>) -> AnyView {
        Box::new(
            VStack::new((
                Text::new(self.0.title.clone()).style(TextStyle::Title).font_weight(Weight::SEMIBOLD),
                self.0.door.description.clone().map(|text| Text::new(text).style(TextStyle::Callout)),
                detail.map(|text| Text::new(text.to_owned()).style(TextStyle::Caption)),
            ))
            .spacing(4.0)
            .align(Align::Stretch),
        )
    }

    fn sign_in_form(&self) -> AnyView {
        let (words, inner) = (&self.0.words, &self.0);
        let (by_key, by_button) = (self.clone(), self.clone());
        let (notice, actions) = self.door_parts();
        Box::new(
            VStack::new((
                self.title(None),
                notice,
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
                actions,
            ))
            .spacing(12.0)
            .align(Align::Stretch),
        )
    }

    fn first_user_form(&self) -> AnyView {
        let (words, inner) = (&self.0.words, &self.0);
        let (by_key, by_button) = (self.clone(), self.clone());
        let (notice, actions) = self.door_parts();
        Box::new(
            VStack::new((
                self.title(Some(&words.first_user)),
                notice,
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
                actions,
            ))
            .spacing(12.0)
            .align(Align::Stretch),
        )
    }

    fn new_password_form(&self, form: &PasswordForm) -> AnyView {
        Box::new(
            VStack::new((
                self.title(Some(&self.0.words.must_change_password)),
                form.clone(),
                self.door_parts().1,
            ))
            .spacing(12.0)
            .align(Align::Stretch),
        )
    }
}

impl View for Gate {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        // The theme at the door is for whoever comes next, and not what the last person chose.
        let inside = matches!(self.0.stage.get(), Stage::Inside(..));
        match self.0.door_theme.get() {
            None => self.0.door_theme.set(Some(*cx.theme())),
            Some(door) if !inside && *cx.theme() != door => cx.theme_state().set(door),
            Some(_) => {}
        }
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
