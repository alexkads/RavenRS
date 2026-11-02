//! The short way to an application.

use std::cell::RefCell;
use std::rc::Rc;

use ravenrs_app::{
    Application, ApplicationBuilder, BusinessObject, MemoryDatabase, ModelError, Module, ObjectSpace,
};
use ravenrs_widgets::{Alert, AnyView, Labels, Localized, Tone};

use crate::editors::EditorRegistry;
use crate::session::{SpaceFactory, Words};
use crate::{AppShell, SystemModule};

/// The language of the screens and of everything inside them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Language {
    /// English.
    #[default]
    English,
    /// Brazilian Portuguese.
    BrazilianPortuguese,
}

impl Language {
    fn words(self) -> Words {
        match self {
            Self::English => Words::default(),
            Self::BrazilianPortuguese => Words::pt_br(),
        }
    }

    fn labels(self) -> Labels {
        match self {
            Self::English => Labels::default(),
            Self::BrazilianPortuguese => Labels::pt_br(),
        }
    }
}

type Seed = Box<dyn FnOnce(&mut dyn ObjectSpace) -> Result<(), String>>;
type Storage = Box<dyn FnOnce(&Application) -> SpaceFactory>;

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
    modules: Vec<Box<dyn FnOnce(ApplicationBuilder) -> ApplicationBuilder>>,
    editors: EditorRegistry,
    seed: Option<Seed>,
    storage: Option<Storage>,
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
            modules: Vec::new(),
            editors: EditorRegistry::default(),
            seed: None,
            storage: None,
        }
    }

    /// The language of its screens.
    pub fn language(mut self, language: Language) -> Self {
        self.language = language;
        self
    }

    /// Adds a record declared as a struct.
    pub fn entity<T: BusinessObject>(mut self) -> Self {
        self.builder = self.builder.entity_of::<T>();
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

    /// Puts it together, or says what is wrong with how it was declared.
    pub fn build(self) -> Result<AnyView, ModelError> {
        let words = self.language.words();
        let mut builder = self.builder.module(SystemModule::with_words(words.clone()));
        for add in self.modules {
            builder = add(builder);
        }
        let application = builder.build()?;
        let factory: SpaceFactory = match self.storage {
            Some(storage) => storage(&application),
            None => {
                let database = MemoryDatabase::new(application.entities().to_vec())?;
                Rc::new(move || Rc::new(RefCell::new(database.space())) as Rc<RefCell<dyn ObjectSpace>>)
            }
        };
        if let Some(seed) = self.seed {
            let space = factory();
            let mut space = space.borrow_mut();
            seed(&mut *space).map_err(ModelError)?;
            space.commit().map_err(|e| ModelError(e.to_string()))?;
        }
        let shell = AppShell::from_factory(application, factory, words, self.editors);
        Ok(Box::new(Localized::new(self.language.labels(), shell)))
    }

    /// Puts it together, showing on the screen what is wrong with how it was declared.
    pub fn into_view(self) -> AnyView {
        self.build().unwrap_or_else(|error| Box::new(Alert::new("RavenRS", error.0).tone(Tone::Danger)))
    }
}
