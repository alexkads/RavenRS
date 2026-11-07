//! The short way to an application.

use std::cell::RefCell;
use std::rc::Rc;

use ravenrs_app::{
    Application, ApplicationBuilder, AttachmentStore, BusinessObject, MemoryDatabase, ModelError, ModelStore,
    Module, ObjectSpace,
};
use ravenrs_app_i18n::{Catalog, Language, Texts};
use ravenrs_widgets::{Alert, AnyView, Labels, Localized, Tone};

use crate::editors::EditorRegistry;
use crate::session::{SpaceFactory, Words};
use crate::{AppShell, SystemModule};

/// Texts given as JSON, by the code of their language, to put over the ones that come with it.
pub(crate) type TextPacks = Vec<(String, String)>;

/// The texts that come with the framework with these over them, in this language.
pub(crate) fn texts_of(language: Language, packs: &[(String, String)]) -> Result<Texts, ModelError> {
    let mut catalog = Catalog::builtin();
    for (code, json) in packs {
        catalog.merge_json(code, json).map_err(|error| ModelError(error.0))?;
    }
    Ok(catalog.texts(language))
}

type Seed = Box<dyn FnOnce(&mut dyn ObjectSpace) -> Result<(), String>>;
type Storage = Box<dyn FnOnce(&Application) -> SpaceFactory>;
type SessionStep = Box<dyn FnOnce(&crate::Session)>;

/// An application from its records: the module with the actions every application needs,
/// the screens, and records in memory unless told otherwise.
///
/// ```
/// use ravenrs_app::{BusinessObject, ObjectSpaceExt};
/// use ravenrs_app_ui::{App, Language};
///
/// #[derive(BusinessObject)]
/// #[object(caption = "Contas", navigation = "Cadastros")]
/// struct Account {
///     #[field(key)]
///     id: i64,
///     #[field(caption = "Nome")]
///     name: String,
/// }
///
/// let window = App::new("Raven Contas")
///     .language(Language::BrazilianPortuguese)
///     .entity::<Account>()
///     .seed(|space| {
///         space.add(&Account { id: 0, name: "Carteira".into() }).map_err(|e| e.to_string())
///     })
///     .build()?;
/// # let _ = window;
/// # Ok::<(), ravenrs_app::ModelError>(())
/// ```
pub struct App {
    builder: ApplicationBuilder,
    language: Language,
    texts: TextPacks,
    modules: Vec<Box<dyn FnOnce(ApplicationBuilder) -> ApplicationBuilder>>,
    editors: EditorRegistry,
    seed: Option<Seed>,
    storage: Option<Storage>,
    list_editors: Vec<(String, crate::list_view::ListEditorFactory)>,
    attachment_store: Option<Rc<dyn AttachmentStore>>,
    model_store: Option<Rc<dyn ModelStore>>,
    steps: Vec<SessionStep>,
    administrator: Option<String>,
    security: Option<(Rc<ravenrs_app::SecurityContext>, ravenrs_app::SpaceMaker)>,
}

impl std::fmt::Debug for App {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("App").field("language", &self.language).finish_non_exhaustive()
    }
}

impl App {
    /// An application with this title, and nothing in it yet.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            builder: Application::builder(title),
            language: Language::default(),
            texts: Vec::new(),
            modules: Vec::new(),
            editors: EditorRegistry::default(),
            seed: None,
            storage: None,
            list_editors: Vec::new(),
            attachment_store: None,
            model_store: None,
            steps: Vec::new(),
            administrator: None,
            security: None,
        }
    }

    /// What the administrator of an installation set in the model, as a difference in JSON. Fields
    /// it adds to an entity reach storage too.
    pub fn administrator_settings(mut self, difference: impl Into<String>) -> Self {
        self.administrator = Some(difference.into());
        self
    }

    /// The records of an entity no storage keeps, for its list and its forms.
    pub fn records_of(
        mut self,
        entity: impl Into<String>,
        source: impl Fn() -> Vec<ravenrs_app::Object> + 'static,
    ) -> Self {
        let entity = entity.into();
        self.steps.push(Box::new(move |session| session.use_records(entity, source)));
        self
    }

    /// The records of a list no storage keeps, inside a form of this entity.
    pub fn collection_of(
        mut self,
        entity: impl Into<String>,
        field: impl Into<String>,
        source: impl Fn(&ravenrs_app::Object) -> Vec<ravenrs_app::Object> + 'static,
    ) -> Self {
        let (entity, field) = (entity.into(), field.into());
        self.steps.push(Box::new(move |session| session.use_collection(entity, field, source)));
        self
    }

    /// The language of its screens.
    pub fn language(mut self, language: Language) -> Self {
        self.language = language;
        self
    }

    /// Adds or replaces words of its screens, and of any component in them, as a flat JSON of keys
    /// in the language of this code: `{ "ui.save": "Gravar" }`. The keys that come with it are in
    /// `ravenrs-app-i18n`; keys of its own, for its modules, go here too, and screens read them with
    /// [`Session::texts`](crate::Session::texts). Given more than once, the last one wins.
    pub fn texts(mut self, language: impl Into<String>, json: impl Into<String>) -> Self {
        self.texts.push((language.into(), json.into()));
        self
    }

    /// Adds a record declared as a struct.
    pub fn entity<T: BusinessObject>(mut self) -> Self {
        self.builder = self.builder.entity_of::<T>();
        self
    }

    /// Adds a record declared at run time.
    pub fn entity_info(mut self, entity: ravenrs_app::EntityInfo) -> Self {
        self.builder = self.builder.entity(entity);
        self
    }

    /// Adds a module of its own, over the one with the actions every application needs.
    pub fn module(mut self, module: impl Module + 'static) -> Self {
        self.modules.push(Box::new(move |builder| builder.module(module)));
        self
    }

    /// Controls of its own for fields.
    pub fn editors(mut self, editors: EditorRegistry) -> Self {
        self.editors = editors;
        self
    }

    /// The model in another language, as a difference in JSON. The one for its
    /// [`language`](Self::language), `en` or `pt-BR`, is the one used. Words of the screens, not of
    /// the model, go to [`texts`](Self::texts).
    pub fn translation(mut self, language: impl Into<String>, difference: impl Into<String>) -> Self {
        self.builder = self.builder.translation(language, difference);
        self
    }

    /// Gives actions and controllers something the application has.
    pub fn service<T: 'static>(mut self, service: T) -> Self {
        self.builder = self.builder.service(service);
        self
    }

    /// Adds a view of other views together.
    pub fn dashboard(mut self, dashboard: ravenrs_app::DashboardInfo) -> Self {
        self.builder = self.builder.dashboard(dashboard);
        self
    }

    /// A way to show a list, for the model to pick by name.
    pub fn list_editor(
        mut self,
        name: impl Into<String>,
        editor: impl Fn(&crate::ListEditorRequest) -> AnyView + 'static,
    ) -> Self {
        self.list_editors.push((name.into(), Rc::new(editor)));
        self
    }

    /// Keeps the content of attached files there, apart from the records.
    pub fn attachment_store(mut self, store: impl AttachmentStore + 'static) -> Self {
        self.attachment_store = Some(Rc::new(store));
        self
    }

    /// Keeps the person's own settings there.
    pub fn model_store(mut self, store: impl ModelStore + 'static) -> Self {
        self.model_store = Some(Rc::new(store));
        self
    }

    /// What is in its records when it opens, kept with a commit once this is done.
    pub fn seed(mut self, seed: impl FnOnce(&mut dyn ObjectSpace) -> Result<(), String> + 'static) -> Self {
        self.seed = Some(Box::new(seed));
        self
    }

    /// Keeps records elsewhere than in memory: this makes the unit of work of each view.
    pub fn storage<S: ObjectSpace + 'static>(mut self, space: impl Fn() -> S + 'static) -> Self {
        self.storage = Some(Box::new(move |_| {
            Rc::new(move || Rc::new(RefCell::new(space())) as Rc<RefCell<dyn ObjectSpace>>)
        }));
        self
    }

    /// Keeps records elsewhere than in memory, in something made from the entities as the model
    /// has them, with the fields its layers added: this makes what makes the unit of work of each view.
    pub fn storage_for<S, F>(mut self, make: impl FnOnce(&[ravenrs_app::EntityInfo]) -> F + 'static) -> Self
    where
        S: ObjectSpace + 'static,
        F: Fn() -> S + 'static,
    {
        self.storage = Some(Box::new(move |application| {
            let space = make(application.entities());
            Rc::new(move || Rc::new(RefCell::new(space())) as Rc<RefCell<dyn ObjectSpace>>)
        }));
        self
    }

    /// For this person: every frame goes by what they can do, and what the application itself
    /// needs in its records is put there through `unsecured`.
    pub(crate) fn secured(
        mut self,
        context: Rc<ravenrs_app::SecurityContext>,
        unsecured: ravenrs_app::SpaceMaker,
    ) -> Self {
        self.seed = None;
        self.security = Some((context, unsecured));
        self
    }

    /// Says who is signed in on the window, and how they leave.
    pub(crate) fn signed_in(mut self, name: String, sign_out: impl Fn() + 'static) -> Self {
        let sign_out: Rc<dyn Fn()> = Rc::new(sign_out);
        self.steps.push(Box::new(move |session| {
            session.use_account(crate::session::Account { name, sign_out });
            session.register_control(crate::security::MODEL_EDITOR, crate::security::model_editor(session));
        }));
        self
    }

    /// Runs this with its session, once it is put together, for what needs a session from outside.
    pub fn with_session(mut self, then: impl FnOnce(&crate::Session) + 'static) -> Self {
        self.steps.push(Box::new(then));
        self
    }

    /// Puts it together, or says what is wrong with how it was declared.
    pub fn build(self) -> Result<AnyView, ModelError> {
        let texts = texts_of(self.language, &self.texts)?;
        let words = Words::from_texts(&texts);
        let mut builder = self.builder.module(SystemModule::with_words(words.clone()));
        for add in self.modules {
            builder = add(builder);
        }
        let mut application = builder.build()?;
        if let Some(text) = &self.administrator {
            let difference = ravenrs_app::Diff::parse(text)?;
            application.layers_mut().put(&ravenrs_app::ModelLayer::Administrator, difference);
        }
        // Before storage is made, for it to know the fields the layers add.
        if let Some(store) = &self.model_store {
            application.layers_mut().load(&ravenrs_app::ModelLayer::User, &**store)?;
        }
        application.set_language(self.language.code())?;
        let unsecured = self.security.as_ref().map(|(_, unsecured)| unsecured.clone());
        if let Some((context, _)) = &self.security {
            application.set_security(context.clone());
        }
        let factory: SpaceFactory = match self.storage {
            Some(storage) => storage(&application),
            None => {
                let database = MemoryDatabase::new(application.entities().to_vec())?;
                Rc::new(move || Rc::new(RefCell::new(database.space())) as Rc<RefCell<dyn ObjectSpace>>)
            }
        };
        match unsecured {
            Some(unsecured) => {
                let mut space = unsecured();
                application.run_initial_records(&mut *space).map_err(ModelError)?;
            }
            None => {
                let space = factory();
                let mut space = space.borrow_mut();
                application.run_initial_records(&mut *space).map_err(ModelError)?;
            }
        }
        if let Some(seed) = self.seed {
            let space = factory();
            let mut space = space.borrow_mut();
            seed(&mut *space).map_err(ModelError)?;
            space.commit().map_err(|e| ModelError(e.to_string()))?;
        }
        let shell = AppShell::from_factory(application, factory, words, self.editors);
        let session = shell.session();
        session.use_texts(texts.clone());
        for (name, editor) in self.list_editors {
            session.register_list_editor(name, move |request| editor(request));
        }
        if let Some(store) = self.attachment_store {
            session.use_attachment_store(store);
        }
        for step in self.steps {
            step(&session);
        }
        if let Some(store) = self.model_store {
            session.use_model_store(store).map_err(ModelError)?;
        }
        Ok(Box::new(Localized::new(Labels::from_texts(&texts), shell)))
    }

    /// Puts it together, showing on the screen what is wrong with how it was declared.
    pub fn into_view(self) -> AnyView {
        self.build().unwrap_or_else(|error| Box::new(Alert::new("RavenRS", error.0).tone(Tone::Danger)))
    }
}
