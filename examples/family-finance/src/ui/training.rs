//! The training films, shipped inside the app, and reporting a problem.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use ravenrs::movie::{
    AssistantLabels, BugLabels, BugReporter, FlightRecorder, Movie, MovieAssistant, MovieCue, MovieLibrary,
};
use ravenrs::prelude::*;
use ravenrs_ai::{CATALOG, ModelFolder, ModelManager, ModelManagerLabels};

use super::assistant::Pending;
use super::store::{Store, heading};
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
/// The room around the player: the sidebar and the page's margins.
const AROUND_THE_PLAYER: f32 = 340.0;

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
        }
    }
}

/// The page with the films.
pub(crate) fn training(state: &TrainingState) -> impl View + use<> {
    let (library, cue) = (state.library.clone(), state.cue.clone());
    ScrollView::new((VStack::new((
        heading(
            "Treinamentos",
            "Pergunte o que quer aprender, ou escolha um assunto e assista no seu ritmo. O filme explica cada ação em voz alta e em uma caixa na tela, e em cada passo espera você clicar no ponto marcado.",
        ),
        responsive(move |size| -> AnyView {
            match &library {
                Some(library) => Box::new(
                    MovieAssistant::new(library.clone())
                        .labels(AssistantLabels::pt_br())
                        .cue(cue.clone())
                        .width((size.width - AROUND_THE_PLAYER).max(480.0)),
                ),
                None => Box::new(
                    Alert::new("Treinamentos indisponíveis", "O arquivo dos filmes não pôde ser lido.")
                        .tone(Tone::Danger),
                ),
            }
        }),
    ))
    .spacing(16.0)
    .align(Align::Stretch),))
    .padding(24.0)
}

/// Saves the report to a file the person sends to the team: nothing here goes
/// anywhere by itself, and the film shows their data.
pub(crate) fn reporter(store: &Store, recorder: &FlightRecorder) -> BugReporter {
    let (store, today) = (store.clone(), store.today);
    BugReporter::new(recorder.clone(), move |report| {
        let name = format!("problema-raven-financas-{today}.ravenbug");
        store.save_file(&name, ("Relato de problema", &["ravenbug"]), &report.to_bytes());
    })
    .labels(BugLabels::pt_br())
    .details(|| {
        vec![
            ("versão".to_owned(), update::CURRENT.to_owned()),
            ("sistema".to_owned(), std::env::consts::OS.to_owned()),
        ]
    })
}
