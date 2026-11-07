//! The training films, shipped inside the app, and reporting a problem.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use ravenrs::movie::{
    AssistantLabels, BugLabels, BugReporter, FlightRecorder, Movie, MovieAssistant, MovieCue, MovieLibrary,
    Narration, RecentErrors,
};
use ravenrs::prelude::*;
use ravenrs_ai::{
    CATALOG, ModelFolder, ModelManager, ModelManagerLabels, VOICE_CATALOG, VoiceLabels, VoiceManager,
};

use super::assistant::Pending;
use super::help;
use super::store::{PAGE_INSETS, Page, Store, content_width};
use crate::update;

/// The films, recorded by `tests/training.rs` from the real app.
const FILMS: [(&str, &[u8]); 6] = [
    ("Visão geral", include_bytes!("../../assets/treinamentos/visao-geral.ravenmovie")),
    ("Lançamentos", include_bytes!("../../assets/treinamentos/lancamentos.ravenmovie")),
    ("Contas a pagar", include_bytes!("../../assets/treinamentos/contas.ravenmovie")),
    ("Agenda", include_bytes!("../../assets/treinamentos/agenda.ravenmovie")),
    ("Orçamentos e relatórios", include_bytes!("../../assets/treinamentos/planejamento.ravenmovie")),
    ("Cadastros e backup", include_bytes!("../../assets/treinamentos/cadastros.ravenmovie")),
];
/// The Treinamentos page, in the app's list of pages.
pub(crate) const TRAINING_PAGE: usize = 8;
/// What takes height outside the page: the top bar and the foot of the window.
const AROUND_THE_PAGE: f32 = 92.0;
/// The room the page's scrollbar takes.
const SCROLLBAR: f32 = 12.0;

/// The films, read once: they are small, but views build often.
pub(crate) struct TrainingState {
    pub(crate) library: Option<MovieLibrary>,
    pub(crate) cue: MovieCue,
    pub(crate) chat_open: State<bool>,
    pub(crate) messages: State<Vec<ChatMessage>>,
    pub(crate) pending: Rc<RefCell<Option<Pending>>>,
    /// The optional AI models, kept beside the database. None with no file to keep
    /// them by, as in a database in memory.
    pub(crate) models: Option<ModelManager>,
    pub(crate) voices: Option<VoiceManager>,
    /// The animated logo of the splash, which also opens and closes each film.
    pub(crate) logo: Option<LottieAnimation>,
}

impl TrainingState {
    pub(crate) fn new(database: Option<&Path>) -> Self {
        // One film that does not open hides the page: they were recorded together.
        let library = FILMS.iter().try_fold(MovieLibrary::new(), |library, (title, bytes)| {
            Movie::from_bytes(bytes).ok().map(|film| library.film(*title, film))
        });
        Self {
            library,
            cue: MovieCue::new(),
            chat_open: State::new(false),
            messages: State::new(Vec::new()),
            pending: Rc::new(RefCell::new(None)),
            models: database.and_then(Path::parent).map(|folder| {
                ModelManager::new(CATALOG, ModelFolder::new(folder.join("modelos")))
                    .labels(ModelManagerLabels::pt_br())
            }),
            voices: database.and_then(Path::parent).map(|folder| {
                VoiceManager::new(VOICE_CATALOG, folder.join("modelos").join("voz"))
                    .labels(VoiceLabels::pt_br())
            }),
            logo: LottieAnimation::from_json(super::opening::ANIMATED_LOGO).ok(),
        }
    }
}

/// What the films say, recorded with the `pm_santa` voice of Kokoro, so that nobody needs the
/// model to hear it.
const NARRATION: &[u8] = include_bytes!("../../assets/treinamentos/narracao.voz");

/// The page with the films.
pub(crate) fn training(state: &TrainingState) -> Page {
    let (library, cue, logo) = (state.library.clone(), state.cue.clone(), state.logo.clone());
    Page::new(
        help::TREINAMENTOS,
        "Pergunte o que quer aprender, ou escolha um assunto e assista no seu ritmo.",
        ScrollView::new((VStack::new((responsive(move |size| -> AnyView {
            match &library {
                Some(library) => Box::new({
                    let assistant = MovieAssistant::new(library.clone());
                    match &logo {
                        Some(logo) => assistant.logo(logo.clone()),
                        None => assistant,
                    }
                    .labels(AssistantLabels::pt_br())
                        .cue(cue.clone())
                        .narration(Narration::new(NARRATION))
                        .width((content_width(size) - SCROLLBAR).max(320.0))
                    .height(size.height - AROUND_THE_PAGE - PAGE_INSETS.top - PAGE_INSETS.bottom)
                }),
                None => Box::new(
                    Alert::new("Treinamentos indisponíveis", "O arquivo dos filmes não pôde ser lido.")
                        .tone(Tone::Danger),
                ),
            }
        }),))
        .spacing(16.0)
        .align(Align::Stretch),))
        .padding(PAGE_INSETS),
    )
}

/// Saves the report to a file the person sends to the team: nothing here goes
/// anywhere by itself, and the film shows their data.
pub(crate) fn reporter(store: &Store, recorder: &FlightRecorder, errors: RecentErrors) -> BugReporter {
    let (store, today) = (store.clone(), store.today);
    BugReporter::new(recorder.clone(), move |report| {
        let name = format!("problema-raven-financas-{today}.ravenbug");
        store.save_file(&name, ("Relato de problema", &["ravenbug"]), &report.to_bytes());
    })
    .labels(BugLabels::pt_br())
    .errors(move || errors.lines())
    .details(|| {
        vec![
            ("versão".to_owned(), update::CURRENT.to_owned()),
            ("sistema".to_owned(), std::env::consts::OS.to_owned()),
        ]
    })
}
